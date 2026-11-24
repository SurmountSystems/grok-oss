//! Plan-image text is tesseract stdout for that image file.
//! This does not describe the image and does not call a vision model.
//!
//! `GROK_TESSERACT_BIN`, when set, is the bundled binary. An empty `PATH`
//! does not skip that binary. When the variable is unset, the first
//! `tesseract` on `PATH` is used. A binary that cannot be started yields
//! [`TESSERACT_MISSING`] and does not invent picture text.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Plan text when the `tesseract` binary cannot be started.
pub(crate) const TESSERACT_MISSING: &str = "tesseract is missing\n";

/// Env var set by the grok-oss Nix wrapper to the store `tesseract` binary.
pub(crate) const GROK_TESSERACT_BIN: &str = "GROK_TESSERACT_BIN";

/// Later steps quote this rule when a model description is not the picture text.
pub(crate) const PICTURE_TEXT_RULE: &str =
    "The text of the picture is the OCR text. When a description disagrees, quote the OCR text.";

/// An image shown to a model, still named by path, plus tesseract stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShownImage {
    pub ocr_text: String,
    pub image_path: PathBuf,
}

impl ShownImage {
    /// The image path is still on the result. OCR failure does not drop it.
    pub(crate) fn image_still_attached(&self) -> bool {
        !self.image_path.as_os_str().is_empty() && self.image_path.is_file()
    }
}

/// Program to run. A non-empty `GROK_TESSERACT_BIN` wins over `PATH`.
fn tesseract_program() -> String {
    match std::env::var(GROK_TESSERACT_BIN) {
        Ok(path) if !path.trim().is_empty() => path,
        _ => "tesseract".to_owned(),
    }
}

/// Run tesseract on `image_path`.
///
/// The fake plan-view `tesseract` records each argument on its own line and
/// prints its marker on stdout. The image path is one argument. `stdout` is
/// tesseract's output target so the recognized text is the process stdout
/// rather than a sibling file.
///
/// Returns that stdout. When the chosen binary cannot be started, returns
/// [`TESSERACT_MISSING`]. Does not fall back to another binary and does not
/// invent picture text.
pub(crate) fn tesseract_stdout_for_plan_image(image_path: &Path) -> String {
    let program = tesseract_program();
    match Command::new(&program)
        .arg(image_path)
        .arg("stdout")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(_) => TESSERACT_MISSING.to_owned(),
    }
}

/// Show `image_path` to a model: OCR stdout plus the same path, still attached.
pub(crate) fn show_image_with_tesseract(image_path: &Path) -> ShownImage {
    ShownImage {
        ocr_text: tesseract_stdout_for_plan_image(image_path),
        image_path: image_path.to_path_buf(),
    }
}

/// Picture text for a later step. OCR wins when `model_description` disagrees.
/// A missing binary stays [`TESSERACT_MISSING`] and is not replaced by the description.
pub(crate) fn picture_text_quoting_ocr(ocr: &str, model_description: &str) -> String {
    let ocr_body = ocr.trim();
    if ocr_body.is_empty() {
        return String::new();
    }
    if ocr_body == TESSERACT_MISSING.trim() || model_description.trim() != ocr_body {
        return ocr.to_owned();
    }
    ocr.to_owned()
}

/// Rule line plus the OCR text later steps must quote. Empty when `ocr` is empty.
pub(crate) fn render_picture_text_block(ocr: &str, model_description: Option<&str>) -> String {
    let quoted = match model_description {
        Some(desc) => picture_text_quoting_ocr(ocr, desc),
        None => ocr.to_owned(),
    };
    if quoted.is_empty() {
        return String::new();
    }
    format!("{PICTURE_TEXT_RULE}\n{quoted}")
}

/// Append the picture-text block for `ocr` onto `body`. Empty OCR leaves `body` unchanged.
pub(crate) fn append_rendered_picture_text(body: &mut String, ocr: &str) {
    let block = render_picture_text_block(ocr, None);
    if block.is_empty() {
        return;
    }
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    if !body.is_empty() {
        body.push('\n');
    }
    body.push_str(&block);
    if !block.ends_with('\n') {
        body.push('\n');
    }
}

/// Append the picture-text block for `image_path` onto `body`.
pub(crate) fn append_ocr_picture_text(body: &mut String, image_path: &Path) {
    let shown = show_image_with_tesseract(image_path);
    append_rendered_picture_text(body, &shown.ocr_text);
}

/// Append `text` to the open plan body. Empty text leaves the body unchanged.
pub(crate) fn append_tesseract_stdout(plan_content: &mut Option<String>, text: &str) {
    if text.is_empty() {
        return;
    }
    let body = plan_content.get_or_insert_with(String::new);
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str(text);
    if !text.ends_with('\n') {
        body.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct RestoreEnv {
        path: Option<String>,
        bundled: Option<String>,
    }

    impl RestoreEnv {
        fn capture() -> Self {
            Self {
                path: std::env::var("PATH").ok(),
                bundled: std::env::var(GROK_TESSERACT_BIN).ok(),
            }
        }
    }

    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            unsafe {
                match &self.path {
                    Some(value) => std::env::set_var("PATH", value),
                    None => std::env::remove_var("PATH"),
                }
                match &self.bundled {
                    Some(value) => std::env::set_var(GROK_TESSERACT_BIN, value),
                    None => std::env::remove_var(GROK_TESSERACT_BIN),
                }
            }
        }
    }

    fn write_executable(path: &Path, script: &str) {
        std::fs::write(path, script).expect("write fake tesseract");
        let mut perms = std::fs::metadata(path)
            .expect("fake tesseract metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).expect("mark fake tesseract executable");
    }

    fn touch_image(dir: &Path) -> PathBuf {
        let image = dir.join("screen.png");
        std::fs::write(&image, b"not-a-real-png").expect("write image");
        image
    }

    /// An image shown to the model includes tesseract stdout and keeps the file.
    /// The assert fails when stdout is absent. A disagreeing description is not the picture text.
    #[test]
    fn shown_image_includes_tesseract_stdout_and_keeps_the_image() {
        let _restore = RestoreEnv::capture();
        let dir = std::env::temp_dir().join(format!(
            "grok-tesseract-shown-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let bin_dir = dir.join("bin");
        std::fs::create_dir(&bin_dir).expect("bin dir");
        let fake = bin_dir.join("tesseract");
        write_executable(
            &fake,
            "#!/bin/sh\nprintf '%s\\n' 'TESSERACT-STDOUT-MARKER'\n",
        );
        let image = touch_image(&dir);
        let path_value = bin_dir.display().to_string();
        unsafe {
            std::env::remove_var(GROK_TESSERACT_BIN);
            std::env::set_var("PATH", &path_value);
        }

        let shown = show_image_with_tesseract(&image);
        assert!(
            shown.ocr_text.contains("TESSERACT-STDOUT-MARKER"),
            "image shown without tesseract stdout: {shown:?}"
        );
        assert!(
            shown.image_still_attached(),
            "image must stay attached: {}",
            shown.image_path.display()
        );
        assert_eq!(shown.image_path, image);
        let quoted = picture_text_quoting_ocr(&shown.ocr_text, "a red square, not the OCR text");
        assert!(
            quoted.contains("TESSERACT-STDOUT-MARKER"),
            "later steps must quote OCR when the description disagrees, got {quoted:?}"
        );
        assert!(
            !quoted.contains("a red square"),
            "a disagreeing description must not replace OCR text, got {quoted:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Empty PATH does not skip `GROK_TESSERACT_BIN`. That binary's stdout is the OCR text.
    #[test]
    fn bundled_binary_runs_when_path_has_no_tesseract() {
        let _restore = RestoreEnv::capture();
        let dir = std::env::temp_dir().join(format!(
            "grok-tesseract-bundled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let fake = dir.join("tesseract");
        write_executable(&fake, "#!/bin/sh\nprintf '%s\\n' 'BUNDLED-OCR-TEXT'\n");
        let image = touch_image(&dir);
        unsafe {
            std::env::set_var("PATH", "");
            std::env::set_var(GROK_TESSERACT_BIN, fake.display().to_string());
        }

        let shown = show_image_with_tesseract(&image);
        assert!(
            shown.ocr_text.contains("BUNDLED-OCR-TEXT"),
            "bundled binary stdout must be the OCR text when PATH has no tesseract, got {shown:?}"
        );
        assert!(
            shown.image_still_attached(),
            "image must stay attached when the bundled binary runs"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A bundled path that cannot start says `tesseract is missing`, keeps the image, and does not invent text.
    #[test]
    fn missing_bundled_binary_says_tesseract_is_missing_and_keeps_the_image() {
        let _restore = RestoreEnv::capture();
        let dir = std::env::temp_dir().join(format!(
            "grok-tesseract-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let bin_dir = dir.join("bin");
        std::fs::create_dir(&bin_dir).expect("bin dir");
        let path_fake = bin_dir.join("tesseract");
        write_executable(
            &path_fake,
            "#!/bin/sh\nprintf '%s\\n' 'invented picture text'\n",
        );
        let image = touch_image(&dir);
        let missing = dir.join("no-such-tesseract");
        unsafe {
            std::env::set_var("PATH", bin_dir.display().to_string());
            std::env::set_var(GROK_TESSERACT_BIN, missing.display().to_string());
        }

        let shown = show_image_with_tesseract(&image);
        assert_eq!(shown.ocr_text, TESSERACT_MISSING);
        assert!(
            shown.ocr_text.contains("tesseract is missing"),
            "missing binary must say tesseract is missing, got {shown:?}"
        );
        assert!(
            shown.image_still_attached(),
            "image must stay attached when the bundled binary cannot start"
        );
        assert!(
            !shown.ocr_text.contains("invented picture text"),
            "a PATH tesseract must not invent picture text when the bundled binary cannot start, got {}",
            shown.ocr_text
        );
        let quoted = picture_text_quoting_ocr(&shown.ocr_text, "a caption the model invented");
        assert_eq!(quoted, TESSERACT_MISSING);
        assert!(!quoted.contains("caption the model invented"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
