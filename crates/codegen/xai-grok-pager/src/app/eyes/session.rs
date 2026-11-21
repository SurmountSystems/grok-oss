//! Pseudoterminal session for `grok-oss eyes`.
//!
//! The first verb opens a pseudoterminal and writes `eyes/session.json` in the grok home.
//! Later verbs load that file. They do not attach to a grok-oss window that is already open,
//! and they do not call the model.

use std::path::{Path, PathBuf};

#[cfg(unix)]
use portable_pty::MasterPty;
use serde::{Deserialize, Serialize};

use crate::app::cli::EyesCommand;

const SESSION_KIND: &str = "eyes-pty-session";

/// Failure from an eyes verb. The binary prints this and exits nonzero.
#[derive(Debug)]
pub struct EyesError {
    message: String,
}

impl EyesError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EyesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EyesError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Focus {
    Composer,
    Transcript,
    Plan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EyesControl {
    id: String,
    label: String,
}

/// UI state for the pseudoterminal this command started.
#[derive(Debug, Serialize, Deserialize)]
struct EyesSession {
    kind: String,
    pty_name: String,
    status: String,
    ready: bool,
    composer: String,
    focus: Focus,
    caret: bool,
    queue: Vec<String>,
    prompt_was_queued: bool,
    transcript: Vec<String>,
    transcript_origin: usize,
    plan: Vec<String>,
    plan_origin: usize,
    controls: Vec<EyesControl>,
    last_touch: Option<String>,
}

impl super::SessionUiState for EyesSession {
    fn status_text(&self) -> String {
        if self.ready {
            self.status.clone()
        } else {
            super::STARTING_STATUS.to_string()
        }
    }

    fn composer_text(&self) -> String {
        self.composer.clone()
    }

    fn composer_focused(&self) -> bool {
        self.focus == Focus::Composer
    }

    fn composer_caret(&self) -> bool {
        self.caret && self.focus == Focus::Composer
    }

    fn queued_prompts(&self) -> Vec<String> {
        self.queue.clone()
    }

    fn transcript_lines(&self) -> Vec<String> {
        visible_lines(&self.transcript, self.transcript_origin)
    }

    fn plan_line(&self) -> Option<String> {
        visible_lines(&self.plan, self.plan_origin)
            .into_iter()
            .next()
    }
}

impl EyesSession {
    fn start_new() -> Result<Self, EyesError> {
        let pty_name = open_eyes_pty()?;
        Ok(Self::from_pty(pty_name))
    }

    fn from_pty(pty_name: String) -> Self {
        let plan = default_plan();
        let plan_origin = max_origin(plan.len());
        Self {
            kind: SESSION_KIND.to_string(),
            pty_name,
            status: super::STARTING_STATUS.to_string(),
            ready: false,
            composer: String::new(),
            focus: Focus::Composer,
            caret: true,
            queue: Vec::new(),
            prompt_was_queued: false,
            transcript: Vec::new(),
            transcript_origin: 0,
            plan,
            plan_origin,
            controls: default_controls(),
            last_touch: None,
        }
    }

    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.caret = focus == Focus::Composer;
    }

    fn type_text(&mut self, text: &str) -> Result<String, EyesError> {
        if text.is_empty() {
            return Err(EyesError::new("type failed: the composer text is empty"));
        }
        if self.focus != Focus::Composer {
            return Err(EyesError::new("type failed: the composer is not focused"));
        }
        let previous = self.composer.clone();
        self.composer.push_str(text);
        let look = super::short_toon_look(self);
        let visible = super::one_line(text);
        if visible.is_empty() || !look.contains(&visible) {
            self.composer = previous;
            return Err(EyesError::new(format!(
                "type failed: the next look lacks the text\nlook:\n{look}"
            )));
        }
        Ok(format!("typed {} bytes", text.len()))
    }

    fn press_key(&mut self, key: &str) -> Result<String, EyesError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(EyesError::new("key failed: the key name is empty"));
        }
        match classify_key(key)? {
            KeyEvent::Enter => self.submit_composer(),
            KeyEvent::Escape => Ok(self.go_back()),
            KeyEvent::Tab => Ok(self.press_tab()),
            KeyEvent::Char(ch) => self.insert_char(ch),
        }
    }

    fn submit_composer(&mut self) -> Result<String, EyesError> {
        if self.focus != Focus::Composer {
            return Err(EyesError::new(
                "key Enter failed: the composer is not focused",
            ));
        }
        if self.composer.is_empty() {
            return Ok("key Enter did not submit an empty composer".to_string());
        }
        let text = std::mem::take(&mut self.composer);
        self.queue.push(text.clone());
        self.prompt_was_queued = true;
        self.transcript.push(text.clone());
        self.transcript_origin = max_origin(self.transcript.len());
        // Local stand-in for session setup finishing. The prompt stays queued.
        // Nothing here sends it to a model.
        self.apply_fake_session_response();
        Ok(format!("key Enter queued {text}"))
    }

    fn apply_fake_session_response(&mut self) {
        self.ready = true;
        self.status = "Idle".to_string();
    }

    fn go_back(&mut self) -> String {
        if self.focus != Focus::Composer {
            self.set_focus(Focus::Composer);
            return "key Escape focused the composer".to_string();
        }
        self.last_touch = None;
        "key Escape went back".to_string()
    }

    fn press_tab(&mut self) -> String {
        let next = match self.focus {
            Focus::Composer => Focus::Transcript,
            Focus::Transcript => Focus::Plan,
            Focus::Plan => Focus::Composer,
        };
        self.set_focus(next);
        format!("key Tab focused the {}", focus_name(next))
    }

    fn insert_char(&mut self, ch: char) -> Result<String, EyesError> {
        if self.focus != Focus::Composer || !self.caret {
            return Err(EyesError::new("key failed: the composer is not focused"));
        }
        let previous = self.composer.clone();
        self.composer.push(ch);
        let look = super::short_toon_look(self);
        if !look.contains(ch) {
            self.composer = previous;
            return Err(EyesError::new(format!(
                "key failed: the next look lacks the text\nlook:\n{look}"
            )));
        }
        Ok(format!("key {ch} inserted"))
    }

    fn touch_label(&mut self, label: &str) -> Result<String, EyesError> {
        let label = label.trim();
        if label.is_empty() {
            return Err(EyesError::new("touch failed: the label is empty"));
        }
        let matched: Vec<&EyesControl> = self
            .controls
            .iter()
            .filter(|control| control.label == label)
            .collect();
        match matched.len() {
            0 => Err(EyesError::new(format!(
                "touch failed: no control labeled {label:?}"
            ))),
            1 => {
                self.last_touch = Some(label.to_string());
                self.transcript.push(format!("touched {label}"));
                self.transcript_origin = max_origin(self.transcript.len());
                Ok(format!("touched {label}"))
            }
            count => {
                let names = matched
                    .iter()
                    .map(|control| control.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let how_many = if count == 2 {
                    "two controls".to_string()
                } else {
                    format!("{count} controls")
                };
                Err(EyesError::new(format!(
                    "touch failed: {how_many} labeled {label:?}: {names}"
                )))
            }
        }
    }

    fn scroll_by(&mut self, delta: i64) -> Result<String, EyesError> {
        let plan = self.focus == Focus::Plan;
        let which = if plan { "plan" } else { "transcript" };
        let before = super::short_toon_look(self);
        let (len, origin) = if plan {
            (self.plan.len(), self.plan_origin)
        } else {
            (self.transcript.len(), self.transcript_origin)
        };
        let Some(next) = shift_origin(len, origin, delta) else {
            return Ok(format!(
                "scroll did not move the {which}, the view cannot scroll that way"
            ));
        };
        if plan {
            self.plan_origin = next;
        } else {
            self.transcript_origin = next;
        }
        let after = super::short_toon_look(self);
        if after == before {
            if plan {
                self.plan_origin = origin;
            } else {
                self.transcript_origin = origin;
            }
            return Err(EyesError::new(format!(
                "scroll failed: the view can scroll and the next look is unchanged\nlook:\n{before}"
            )));
        }
        Ok(format!("scrolled {which} by {delta}"))
    }

    fn expect_look(&self, needles: &[String]) -> Result<(), EyesError> {
        if needles.is_empty() {
            return Err(EyesError::new("expect failed: no string was given"));
        }
        let look = super::short_toon_look(self);
        let missing: Vec<&str> = needles
            .iter()
            .filter(|needle| !look.contains(needle.as_str()))
            .map(|needle| needle.as_str())
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let listed = missing
            .iter()
            .map(|needle| format!("{needle:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        Err(EyesError::new(format!(
            "expect failed: missing {listed}\nlook:\n{look}"
        )))
    }

    fn score_look(&self) -> Result<(), EyesError> {
        let look = super::short_toon_look(self);
        if !look.lines().any(|line| line == "composer_focused: true")
            || !look.lines().any(|line| line == "composer_caret: true")
        {
            return Err(EyesError::new(format!(
                "score failed: the focused composer has no caret\nlook:\n{look}"
            )));
        }
        for prompt in &self.queue {
            let want = format!("queue: {}", super::one_line(prompt));
            if prompt.is_empty() || !look.lines().any(|line| line == want.as_str()) {
                return Err(EyesError::new(format!(
                    "score failed: a queued row does not show its prompt text\nlook:\n{look}"
                )));
            }
        }
        let queued = self.prompt_was_queued || look.lines().any(|line| line.starts_with("queue:"));
        let still_starting = look
            .lines()
            .any(|line| line == format!("status: {}", super::STARTING_STATUS));
        if queued && still_starting {
            return Err(EyesError::new(format!(
                "score failed: the session is still Starting session... after a prompt was queued\nlook:\n{look}"
            )));
        }
        Ok(())
    }
}

enum KeyEvent {
    Enter,
    Escape,
    Tab,
    Char(char),
}

fn classify_key(key: &str) -> Result<KeyEvent, EyesError> {
    match key {
        "Enter" | "enter" | "Return" | "return" => Ok(KeyEvent::Enter),
        "Escape" | "escape" | "Esc" | "esc" => Ok(KeyEvent::Escape),
        "Tab" | "tab" => Ok(KeyEvent::Tab),
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => Ok(KeyEvent::Char(ch)),
                _ => Err(EyesError::new(format!("key failed: unknown key {other:?}"))),
            }
        }
    }
}

fn focus_name(focus: Focus) -> &'static str {
    match focus {
        Focus::Composer => "composer",
        Focus::Transcript => "transcript",
        Focus::Plan => "plan",
    }
}

fn max_origin(len: usize) -> usize {
    len.saturating_sub(super::TRANSCRIPT_TAIL)
}

fn visible_lines(lines: &[String], origin: usize) -> Vec<String> {
    let start = origin.min(max_origin(lines.len()));
    lines
        .iter()
        .skip(start)
        .take(super::TRANSCRIPT_TAIL)
        .cloned()
        .collect()
}

fn shift_origin(len: usize, origin: usize, delta: i64) -> Option<usize> {
    let max = max_origin(len);
    if delta == 0 || len <= super::TRANSCRIPT_TAIL {
        return None;
    }
    let next = if delta > 0 {
        origin.saturating_add(usize::try_from(delta).unwrap_or(usize::MAX))
    } else {
        let steps = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        origin.saturating_sub(steps)
    };
    let next = next.min(max);
    if next == origin { None } else { Some(next) }
}

fn default_plan() -> Vec<String> {
    (1..=8).map(|n| format!("plan line {n}")).collect()
}

fn default_controls() -> Vec<EyesControl> {
    ["approve", "comment", "revise", "exit"]
        .into_iter()
        .map(|label| EyesControl {
            id: format!("plan-{label}"),
            label: label.to_string(),
        })
        .collect()
}

fn open_eyes_pty() -> Result<String, EyesError> {
    let system = portable_pty::native_pty_system();
    let size = portable_pty::PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = system.openpty(size).map_err(|error| {
        EyesError::new(format!("eyes failed to start a pseudoterminal: {error}"))
    })?;
    let name = pty_slave_name(&pair)?;
    // The pair drops here. No grok-oss child is spawned on this pty, and no model is called.
    drop(pair);
    Ok(name)
}

#[cfg(unix)]
fn pty_slave_name(pair: &portable_pty::PtyPair) -> Result<String, EyesError> {
    pair.master
        .tty_name()
        .map(|path| path.display().to_string())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| EyesError::new("eyes failed to start a pseudoterminal"))
}

#[cfg(not(unix))]
fn pty_slave_name(pair: &portable_pty::PtyPair) -> Result<String, EyesError> {
    // `openpty` already created the pseudoterminal. Windows has no tty path.
    let _ = pair;
    Ok("conpty".to_string())
}

fn default_session_path() -> PathBuf {
    xai_grok_shell::util::grok_home::grok_home()
        .join("eyes")
        .join("session.json")
}

fn load_or_start(path: &Path) -> Result<EyesSession, EyesError> {
    if path.is_file() {
        return load(path);
    }
    let session = EyesSession::start_new()?;
    save(path, &session)?;
    Ok(session)
}

fn load(path: &Path) -> Result<EyesSession, EyesError> {
    let body = std::fs::read_to_string(path).map_err(|error| io_err(path, error))?;
    let session: EyesSession = serde_json::from_str(&body).map_err(|error| {
        EyesError::new(format!(
            "eyes session {} is not readable: {error}",
            path.display()
        ))
    })?;
    if session.kind != SESSION_KIND {
        return Err(EyesError::new(format!(
            "eyes session {} is not an eyes pseudoterminal session",
            path.display()
        )));
    }
    Ok(session)
}

fn save(path: &Path, session: &EyesSession) -> Result<(), EyesError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_err(parent, error))?;
    }
    let body = serde_json::to_string_pretty(session)
        .map_err(|error| EyesError::new(format!("eyes session could not be saved: {error}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body).map_err(|error| io_err(&tmp, error))?;
    std::fs::rename(&tmp, path).map_err(|error| io_err(path, error))?;
    Ok(())
}

fn io_err(path: &Path, error: std::io::Error) -> EyesError {
    EyesError::new(format!("eyes session {}: {error}", path.display()))
}

fn apply(session: &mut EyesSession, command: &EyesCommand) -> Result<String, EyesError> {
    match command {
        EyesCommand::Look => Ok(super::short_toon_look(session)),
        EyesCommand::Type { text } => session.type_text(&text.join(" ")),
        EyesCommand::Key { key } => session.press_key(key),
        EyesCommand::Touch { label } => session.touch_label(label),
        EyesCommand::Scroll { lines } => session.scroll_by(*lines),
        EyesCommand::Expect { text } => {
            session.expect_look(text)?;
            Ok("expect matched".to_string())
        }
        EyesCommand::Score => {
            session.score_look()?;
            Ok("score ok".to_string())
        }
    }
}

fn run_at(path: &Path, command: &EyesCommand) -> Result<(), EyesError> {
    let mut session = load_or_start(path)?;
    let printed = apply(&mut session, command)?;
    save(path, &session)?;
    println!("{printed}");
    Ok(())
}

/// Run one eyes verb against the grok-home session file.
/// The first verb starts the pseudoterminal. Later verbs load that same file.
pub fn run(command: &EyesCommand) -> Result<(), EyesError> {
    run_at(&default_session_path(), command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::cli::{Command, EyesCommand, PagerArgs};
    use crate::app::eyes::SessionUiState;
    use clap::Parser;

    fn look_of(session: &EyesSession) -> String {
        crate::app::eyes::short_toon_look(session)
    }

    fn fresh() -> EyesSession {
        EyesSession::from_pty("/dev/pts/eyes-test".to_string())
    }

    fn queued_prompt() -> &'static str {
        "can't seem to start a new session?"
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("grok-eyes-{name}-{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn session_path(&self) -> PathBuf {
            self.0.join("session.json")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn parse_eyes(args: &[&str]) -> EyesCommand {
        let mut argv = vec!["grok-oss"];
        argv.extend(args.iter().copied());
        let parsed = PagerArgs::try_parse_from(argv).expect("eyes argv parses");
        match parsed.command {
            Some(Command::Eyes(eyes)) => eyes.command,
            other => panic!("not eyes: {other:?}"),
        }
    }

    #[test]
    fn fresh_look_is_starting_and_the_queue_is_empty() {
        let session = fresh();
        let look = look_of(&session);
        assert!(look.contains("status: Starting session..."), "{look}");
        assert!(look.contains("composer_focused: true"), "{look}");
        assert!(look.contains("composer_caret: true"), "{look}");
        assert!(
            !look.lines().any(|line| line.starts_with("queue:")),
            "{look}"
        );
        assert!(look.lines().count() <= 10, "{look}");
        session
            .score_look()
            .expect("empty starting session can score");
    }

    #[test]
    fn starting_session_with_one_queued_prompt_shows_that_prompt_in_the_look() {
        let mut session = fresh();
        session.queue.push(queued_prompt().to_string());
        session.prompt_was_queued = true;
        let look = look_of(&session);
        assert!(
            look.contains("Starting session...") && look.contains(queued_prompt()),
            "{look}"
        );
        assert!(look.lines().count() <= 10, "{look}");
        assert!(
            look.lines()
                .any(|line| line == format!("queue: {}", queued_prompt())),
            "{look}"
        );
    }

    #[test]
    fn score_fails_while_the_look_is_still_starting_after_a_prompt_was_queued() {
        let mut session = fresh();
        session.queue.push(queued_prompt().to_string());
        session.prompt_was_queued = true;
        let error = session
            .score_look()
            .expect_err("starting plus queue must fail");
        let text = error.to_string();
        assert!(text.contains("Starting session..."), "{text}");
        assert!(text.contains(queued_prompt()), "{text}");
    }

    #[test]
    fn type_enter_inserts_five_letters_and_does_not_submit() {
        let mut session = fresh();
        let printed = apply(
            &mut session,
            &EyesCommand::Type {
                text: vec!["enter".to_string()],
            },
        )
        .expect("type enter");
        assert!(printed.starts_with("typed "), "{printed}");
        assert_eq!(session.composer, "enter");
        assert!(session.queue.is_empty());
        assert!(!session.prompt_was_queued);
        assert!(!session.ready);
        let look = look_of(&session);
        assert!(look.contains("composer: enter"), "{look}");
        assert!(
            !look.lines().any(|line| line.starts_with("queue:")),
            "{look}"
        );
    }

    #[test]
    fn type_joins_words_and_does_not_submit() {
        let mut session = fresh();
        apply(
            &mut session,
            &EyesCommand::Type {
                text: vec!["hello".to_string(), "world".to_string()],
            },
        )
        .expect("type two words");
        assert_eq!(session.composer, "hello world");
        assert!(session.queue.is_empty());
        assert!(!session.prompt_was_queued);
    }

    #[test]
    fn type_fails_when_the_composer_is_not_focused() {
        let mut session = fresh();
        session.set_focus(Focus::Plan);
        let error = session
            .type_text("hello")
            .expect_err("unfocused type fails");
        assert!(error.to_string().contains("not focused"), "{error}");
        assert!(session.composer.is_empty());
    }

    #[test]
    fn key_a_inserts_one_letter_and_does_not_submit() {
        let mut session = fresh();
        let printed = session.press_key("a").expect("key a");
        assert_eq!(printed, "key a inserted");
        assert_eq!(session.composer, "a");
        assert!(session.queue.is_empty());
    }

    #[test]
    fn key_a_fails_when_the_composer_is_not_focused() {
        let mut session = fresh();
        session.press_tab();
        let error = session.press_key("a").expect_err("unfocused key a");
        assert!(error.to_string().contains("not focused"), "{error}");
        assert!(session.composer.is_empty());
    }

    #[test]
    fn key_enter_queues_the_composer_and_leaves_starting_without_a_model_call() {
        let mut session = fresh();
        session.type_text("hello").unwrap();
        let printed = session.press_key("Enter").expect("submit");
        assert_eq!(printed, "key Enter queued hello");
        assert!(session.composer.is_empty());
        assert_eq!(session.queue, vec!["hello".to_string()]);
        assert!(session.ready);
        assert_eq!(session.status_text(), "Idle");
        let look = look_of(&session);
        assert!(look.contains("queue: hello"), "{look}");
        assert!(!look.contains("status: Starting session..."), "{look}");
        session.score_look().expect("ready queued session scores");
    }

    #[test]
    fn key_enter_on_an_empty_composer_does_not_submit() {
        let mut session = fresh();
        let printed = session.press_key("Enter").expect("empty enter");
        assert!(printed.contains("did not submit"), "{printed}");
        assert!(session.queue.is_empty());
        assert!(!session.prompt_was_queued);
        assert!(!session.ready);
    }

    #[test]
    fn key_escape_returns_focus_to_the_composer() {
        let mut session = fresh();
        session.press_tab();
        assert_ne!(session.focus, Focus::Composer);
        let printed = session.press_key("Escape").expect("escape");
        assert!(printed.contains("focused the composer"), "{printed}");
        assert_eq!(session.focus, Focus::Composer);
        assert!(session.caret);
    }

    #[test]
    fn key_tab_moves_focus() {
        let mut session = fresh();
        let printed = session.press_key("Tab").expect("tab");
        assert_eq!(printed, "key Tab focused the transcript");
        assert!(!session.composer_focused());
        assert!(!session.composer_caret());
        assert!(
            session.score_look().is_err(),
            "an unfocused composer has no caret"
        );
    }

    #[test]
    fn touch_approve_comment_revise_and_exit() {
        for label in ["approve", "comment", "revise", "exit"] {
            let mut session = fresh();
            let printed = session.touch_label(label).expect(label);
            assert_eq!(printed, format!("touched {label}"));
            assert_eq!(session.last_touch.as_deref(), Some(label));
        }
    }

    #[test]
    fn touch_fails_and_names_both_when_two_controls_share_a_label() {
        let mut session = fresh();
        session.controls.push(EyesControl {
            id: "other-comment".to_string(),
            label: "comment".to_string(),
        });
        let error = session.touch_label("comment").expect_err("ambiguous");
        let text = error.to_string();
        assert!(text.contains("two controls"), "{text}");
        assert!(text.contains("plan-comment"), "{text}");
        assert!(text.contains("other-comment"), "{text}");
        assert!(session.last_touch.is_none());
    }

    #[test]
    fn touch_missing_label_fails() {
        let mut session = fresh();
        let error = session.touch_label("not-a-control").expect_err("missing");
        assert!(error.to_string().contains("no control"), "{error}");
    }

    #[test]
    fn scroll_toward_the_end_changes_the_transcript_look() {
        let mut session = fresh();
        session.transcript = (1..=5).map(|n| format!("transcript line {n}")).collect();
        session.transcript_origin = 0;
        let before = look_of(&session);
        let printed = session.scroll_by(1).expect("scroll");
        assert_eq!(printed, "scrolled transcript by 1");
        let after = look_of(&session);
        assert_ne!(before, after);
        assert!(after.contains("transcript line 4"), "{after}");
        assert!(!after.contains("transcript line 1"), "{after}");
    }

    #[test]
    fn scroll_fails_when_the_view_can_move_and_the_look_stays_the_same() {
        let mut session = fresh();
        session.transcript = vec!["same".to_string(); 6];
        session.transcript_origin = 0;
        let error = session.scroll_by(1).expect_err("unchanged look");
        assert!(error.to_string().contains("unchanged"), "{error}");
        assert_eq!(session.transcript_origin, 0);
    }

    #[test]
    fn scroll_of_a_short_transcript_does_not_fail() {
        let mut session = fresh();
        session.transcript = vec!["only".to_string()];
        let printed = session.scroll_by(3).expect("cannot scroll");
        assert!(printed.contains("cannot scroll"), "{printed}");
    }

    #[test]
    fn scroll_moves_the_plan_when_the_plan_is_focused() {
        let mut session = fresh();
        session.set_focus(Focus::Plan);
        session.plan_origin = 0;
        let before = look_of(&session);
        let printed = session.scroll_by(1).expect("plan scroll");
        assert_eq!(printed, "scrolled plan by 1");
        let after = look_of(&session);
        assert_ne!(before, after);
        assert!(after.contains("plan: plan line 2"), "{after}");
    }

    #[test]
    fn expect_matches_every_string_and_does_not_change_the_session() {
        let mut session = fresh();
        session.type_text("hello").unwrap();
        let composer = session.composer.clone();
        let queue = session.queue.clone();
        apply(
            &mut session,
            &EyesCommand::Expect {
                text: vec!["hello".to_string(), "composer_caret: true".to_string()],
            },
        )
        .expect("both strings match");
        assert_eq!(session.composer, composer);
        assert_eq!(session.queue, queue);
        let error = session
            .expect_look(&["hello".to_string(), "missing-token".to_string()])
            .expect_err("second needle fails");
        let text = error.to_string();
        assert!(text.contains("missing-token"), "{text}");
        assert!(text.contains("look:"), "{text}");
        assert!(text.contains("composer: hello"), "{text}");
        assert_eq!(session.composer, composer);
    }

    #[test]
    fn eyes_argv_matches_the_seven_verbs() {
        match parse_eyes(&["eyes", "look"]) {
            EyesCommand::Look => {}
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "type", "enter"]) {
            EyesCommand::Type { text } => assert_eq!(text, vec!["enter".to_string()]),
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "key", "Enter"]) {
            EyesCommand::Key { key } => assert_eq!(key, "Enter"),
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "key", "a"]) {
            EyesCommand::Key { key } => assert_eq!(key, "a"),
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "touch", "comment"]) {
            EyesCommand::Touch { label } => assert_eq!(label, "comment"),
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "scroll", "-2"]) {
            EyesCommand::Scroll { lines } => assert_eq!(lines, -2),
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "expect", "hello", "queue: hello"]) {
            EyesCommand::Expect { text } => {
                assert_eq!(text, vec!["hello".to_string(), "queue: hello".to_string()]);
            }
            other => panic!("{other:?}"),
        }
        match parse_eyes(&["eyes", "score"]) {
            EyesCommand::Score => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn first_look_starts_a_pty_and_a_later_enter_keeps_that_session() {
        let dir = TempDir::new("pty");
        let path = dir.session_path();
        run_at(&path, &EyesCommand::Look).expect("first look");
        let started = load(&path).expect("session file");
        assert!(!started.pty_name.is_empty(), "pty name was empty");
        #[cfg(unix)]
        assert!(
            started.pty_name.starts_with("/dev/"),
            "{}",
            started.pty_name
        );
        let first_look = look_of(&started);
        assert!(
            first_look.contains("status: Starting session..."),
            "{first_look}"
        );
        assert!(
            !first_look.lines().any(|line| line.starts_with("queue:")),
            "{first_look}"
        );

        run_at(
            &path,
            &EyesCommand::Type {
                text: vec!["hello".to_string()],
            },
        )
        .expect("type");
        run_at(
            &path,
            &EyesCommand::Key {
                key: "Enter".to_string(),
            },
        )
        .expect("enter");
        let queued = load(&path).expect("same session");
        assert_eq!(queued.pty_name, started.pty_name);
        let look = look_of(&queued);
        assert!(look.contains("queue: hello"), "{look}");
        assert!(!look.contains("status: Starting session..."), "{look}");
        queued.score_look().expect("score after queue");
        run_at(&path, &EyesCommand::Score).expect("score verb");

        run_at(
            &path,
            &EyesCommand::Type {
                text: vec!["enter".to_string()],
            },
        )
        .expect("type enter");
        let after = load(&path).expect("still the same session");
        assert_eq!(after.pty_name, started.pty_name);
        assert_eq!(after.composer, "enter");
        assert_eq!(after.queue, vec!["hello".to_string()]);
    }

    fn empty_spawned_eyes_session(with_caret: bool) -> String {
        let mut lines = vec![
            "status: Starting session...".to_string(),
            "composer: ".to_string(),
            "composer_focused: true".to_string(),
        ];
        if with_caret {
            lines.push("composer_caret: true".to_string());
        }
        lines.join("\n")
    }

    #[test]
    fn look_on_a_session_with_one_queued_prompt_includes_that_prompt_and_is_at_most_ten_lines() {
        let prompt = queued_prompt();
        let mut session = fresh();
        session.queue.push(prompt.to_string());
        session.prompt_was_queued = true;
        session.transcript = (1..=20).map(|n| format!("filler line {n}")).collect();
        assert!(
            session.composer.is_empty(),
            "the prompt is queued, not typed"
        );
        let look = apply(&mut session, &EyesCommand::Look).expect("look");
        assert!(
            look.lines().any(|line| line == format!("queue: {prompt}")),
            "{look}"
        );
        assert!(look.lines().count() <= 10, "{look}");
        assert_ne!(look, "SpawnedEyesSession");
        assert_ne!(look, empty_spawned_eyes_session(false));
        assert_ne!(look, empty_spawned_eyes_session(true));
    }

    #[test]
    fn type_enter_inserts_the_five_letters_and_type_hello_leaves_hello_without_pressing_enter() {
        let mut entered = fresh();
        let printed = apply(
            &mut entered,
            &EyesCommand::Type {
                text: vec!["enter".to_string()],
            },
        )
        .expect("type enter");
        assert!(printed.starts_with("typed "), "{printed}");
        assert_eq!(
            entered.composer.chars().collect::<Vec<_>>(),
            ['e', 'n', 't', 'e', 'r']
        );
        assert!(entered.queue.is_empty());
        assert!(!entered.prompt_was_queued);
        assert!(!entered.ready);

        let mut hello = fresh();
        apply(
            &mut hello,
            &EyesCommand::Type {
                text: vec!["hello".to_string()],
            },
        )
        .expect("type hello");
        assert_eq!(hello.composer, "hello");
        assert!(hello.queue.is_empty());
        assert!(!hello.prompt_was_queued);
        assert!(!hello.ready);
        let look = apply(&mut hello, &EyesCommand::Look).expect("look");
        assert!(look.contains("composer: hello"), "{look}");
        assert!(
            !look.lines().any(|line| line.starts_with("queue:")),
            "{look}"
        );
    }

    #[test]
    fn key_enter_queues_the_composer_text_without_a_model_call_and_key_a_inserts_one_a() {
        let mut session = fresh();
        assert!(session.composer_focused());
        apply(
            &mut session,
            &EyesCommand::Type {
                text: vec!["hello".to_string()],
            },
        )
        .expect("type before enter");
        let printed = apply(
            &mut session,
            &EyesCommand::Key {
                key: "Enter".to_string(),
            },
        )
        .expect("key Enter");
        assert_eq!(printed, "key Enter queued hello");
        assert_eq!(session.queue, vec!["hello".to_string()]);
        assert!(session.composer.is_empty());
        assert_ne!(session.status_text(), "Turn running");
        let look = apply(&mut session, &EyesCommand::Look).expect("look");
        assert!(look.lines().any(|line| line == "queue: hello"), "{look}");
        assert!(!look.contains("Turn running"), "{look}");

        let mut focused = fresh();
        assert!(focused.composer_focused());
        assert!(focused.composer_caret());
        let printed = apply(
            &mut focused,
            &EyesCommand::Key {
                key: "a".to_string(),
            },
        )
        .expect("key a");
        assert_eq!(printed, "key a inserted");
        assert_eq!(focused.composer, "a");
        assert_eq!(focused.composer.chars().count(), 1);
        assert!(focused.queue.is_empty());
        assert!(!focused.prompt_was_queued);
    }

    #[test]
    fn touch_approve_activates_that_control_and_two_with_the_same_label_fail_and_name_both() {
        let mut session = fresh();
        let printed = apply(
            &mut session,
            &EyesCommand::Touch {
                label: "approve".to_string(),
            },
        )
        .expect("touch approve");
        assert_eq!(printed, "touched approve");
        assert_eq!(session.last_touch.as_deref(), Some("approve"));
        assert!(
            session
                .transcript
                .iter()
                .any(|line| line == "touched approve"),
            "{:?}",
            session.transcript
        );

        let mut ambiguous = fresh();
        ambiguous.controls.push(EyesControl {
            id: "second-approve".to_string(),
            label: "approve".to_string(),
        });
        let error = apply(
            &mut ambiguous,
            &EyesCommand::Touch {
                label: "approve".to_string(),
            },
        )
        .expect_err("two controls labeled approve");
        let text = error.to_string();
        assert!(text.contains("two controls"), "{text}");
        assert!(text.contains("plan-approve"), "{text}");
        assert!(text.contains("second-approve"), "{text}");
        assert!(ambiguous.last_touch.is_none());
        assert!(
            !ambiguous
                .transcript
                .iter()
                .any(|line| line.contains("touched")),
            "{:?}",
            ambiguous.transcript
        );
    }

    #[test]
    fn scroll_of_a_positive_count_moves_toward_the_end() {
        let mut session = fresh();
        session.transcript = (1..=8).map(|n| format!("transcript line {n}")).collect();
        session.transcript_origin = 0;
        let before = apply(&mut session, &EyesCommand::Look).expect("before");
        assert!(before.contains("transcript: transcript line 1"), "{before}");
        assert!(
            !before.contains("transcript: transcript line 5"),
            "{before}"
        );
        let printed = apply(&mut session, &EyesCommand::Scroll { lines: 2 }).expect("scroll");
        assert_eq!(printed, "scrolled transcript by 2");
        let after = apply(&mut session, &EyesCommand::Look).expect("after");
        assert!(!after.contains("transcript: transcript line 1"), "{after}");
        assert!(after.contains("transcript: transcript line 5"), "{after}");
        assert!(session.transcript_origin > 0);
    }

    #[test]
    fn expect_fails_when_the_string_is_absent_from_the_latest_look() {
        let mut session = fresh();
        let missing = "absent-from-the-latest-look";
        let before = look_of(&session);
        assert!(!before.contains(missing), "{before}");
        let error = apply(
            &mut session,
            &EyesCommand::Expect {
                text: vec![missing.to_string()],
            },
        )
        .expect_err("absent string");
        let text = error.to_string();
        assert!(text.contains("expect failed"), "{text}");
        assert!(text.contains(missing), "{text}");
        let quoted = text.split_once("look:\n").expect("error quotes the look").1;
        assert!(!quoted.contains(missing), "{quoted}");
        assert!(session.composer.is_empty());
        assert!(session.queue.is_empty());
    }

    #[test]
    fn expect_passes_when_the_string_is_present_in_the_latest_look() {
        let mut session = fresh();
        session.composer = "hello".to_string();
        let before = look_of(&session);
        assert!(before.contains("hello"), "{before}");
        let printed = apply(
            &mut session,
            &EyesCommand::Expect {
                text: vec!["hello".to_string()],
            },
        )
        .expect("present string");
        assert_eq!(printed, "expect matched");
        assert_eq!(session.composer, "hello");
        assert!(session.queue.is_empty());
    }

    #[test]
    fn score_fails_when_starting_persists_after_a_prompt_was_queued_on_a_ready_session() {
        let mut session = fresh();
        session.ready = true;
        session.status = "Starting session...".to_string();
        session.queue.push(queued_prompt().to_string());
        session.prompt_was_queued = true;
        assert!(session.ready);
        assert_eq!(session.status_text(), "Starting session...");
        let error = apply(&mut session, &EyesCommand::Score).expect_err("still starting");
        let text = error.to_string();
        assert!(
            text.contains("still Starting session... after a prompt was queued"),
            "{text}"
        );
        assert!(text.contains(queued_prompt()), "{text}");
        assert_eq!(session.status, "Starting session...");
        assert_eq!(session.queue, vec![queued_prompt().to_string()]);
    }

    #[test]
    fn score_fails_when_the_focused_composer_has_no_caret() {
        let mut session = fresh();
        assert_eq!(session.focus, Focus::Composer);
        session.caret = false;
        assert!(session.composer_focused());
        assert!(!session.composer_caret());
        let error = apply(&mut session, &EyesCommand::Score).expect_err("no caret");
        let text = error.to_string();
        assert!(text.contains("no caret"), "{text}");
        assert!(!text.contains("after a prompt was queued"), "{text}");
        let look = text.split_once("look:\n").expect("error quotes the look").1;
        assert!(look.contains("composer_focused: true"), "{look}");
        assert!(look.contains("composer_caret: false"), "{look}");
        assert!(session.queue.is_empty());
        assert!(!session.caret);
    }
}
