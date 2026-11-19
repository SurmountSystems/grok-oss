//! Plan-image text is the stdout of the first `tesseract` on PATH.
//! This does not describe the image and does not call a vision model.

use std::path::Path;
use std::process::{Command, Stdio};

/// Plan text when the `tesseract` binary cannot be started.
pub(crate) const TESSERACT_MISSING: &str = "tesseract is missing\n";

/// Run the first `tesseract` on PATH.
///
/// The fake plan-view `tesseract` records each argument on its own line and
/// prints its marker on stdout. The image path is one argument. `stdout` is
/// tesseract's output target so the recognized text is the process stdout
/// rather than a sibling file.
///
/// Returns that stdout. When the binary cannot be started, returns
/// [`TESSERACT_MISSING`].
pub(crate) fn tesseract_stdout_for_plan_image(image_path: &Path) -> String {
    match Command::new("tesseract")
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
