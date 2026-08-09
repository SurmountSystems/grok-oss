//! Turn status line — single-row widget showing current turn activity.
//!
//! Layout: `⠧ Run command 0.2s         1m20s ⇣12k [pause] [stop]`
//!
//! - Spinner (left, slowed to ~7.5fps)
//! - Activity label (colored per activity type, truncates if needed)
//! - Phase timer `Xs` (gray, never truncates)
//! - Queued-send hint `· N queued — Enter to interject` (gray, sendable waits only)
//! - Fill space
//! - Turn timer `Xm Ys` and optional token count `⇣Nk` (right-aligned, gray)
//! - Pause button `[pause]` / `[resume]` (quiet white on hover; global pause)
//! - Cancel button `[stop]` (right-aligned, red on hover; hard cancel)
//!
//! Hidden when idle (0 height) unless watchers, drain-blocked, starting
//! session, or global work pause is active. Appears between scrollback and
//! prompt.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;
use xai_grok_workspace::permission::mcp_pretty_name_if_qualified;

use crate::acp::tracker::{TurnActivity, WaitingReason};
use crate::app::agent::{AgentCommand, AgentState};
use crate::app::agent_view::McpInitProgress;
use crate::render::line_utils::truncate_str;
use crate::theme::Theme;

/// Show each spinner frame for this many animation ticks.
/// At ~30fps, 4 ticks = ~133ms per frame = ~7.5 spinner fps.
pub(crate) const SPINNER_DIVISOR: u64 = 4;

/// Show each monitor-pulse frame for this many animation ticks — twice the
/// [`SPINNER_DIVISOR`] dwell (~3.75 fps). The idle still-running cue should
/// breathe calmly rather than read like the active turn spinner, so its
/// `○ ◎ ◉ ◎` cycle runs at roughly half the speed (~1.07s per loop).
pub(crate) const MONITOR_PULSE_DIVISOR: u64 = 8;

/// Pulse speed for every "waiting on you" diamond — the drain-blocked
/// status, the pending-user-input status, and the plan-approval status
/// all share this cadence. `pulse_brightness` returns `sin²(tick*speed)`,
/// which has period π, so at ~30fps this is ~1.3s per cycle
/// (`π / (0.08 * 30) ≈ 1.31`).
///
/// Always route diamond rendering through [`pending_diamond_color`] so
/// the three call sites can never silently drift apart.
pub(crate) const USER_WAITING_PULSE_SPEED: f32 = 0.08;

/// Compute the pulsing diamond color for any "waiting on you" cue.
///
/// Default themes blend `accent` toward `theme.bg_base` using a `sin²`
/// pulse driven by [`USER_WAITING_PULSE_SPEED`]. Brightness ranges from
/// 0.3 (dim) to 1.0 (full accent) so the diamond stays visible at the
/// trough.
///
/// Under DOGE, solid steps only: full `accent` on the bright half of the
/// cycle, pure black (`bg_base`) on the dim half — no mid-channel gray
/// blend (pure 8-colour law).
///
/// Pass `theme.accent_user` for user-input waits (permission prompts,
/// `ask_user_question`, the drain-blocked idle status) and
/// `theme.accent_plan` for plan-approval waits.
pub(crate) fn pending_diamond_color(theme: &Theme, accent: Color, tick: u64) -> Color {
    let brightness = crate::theme::pulse_brightness(tick, USER_WAITING_PULSE_SPEED);
    if crate::theme::Theme::current_kind() == crate::theme::ThemeKind::Doge {
        if brightness >= 0.5 {
            accent
        } else {
            theme.bg_base
        }
    } else {
        crate::render::color::blend_color(theme.bg_base, accent, 0.3 + brightness * 0.7)
            .unwrap_or(accent)
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// Output from rendering the turn status line.
#[derive(Debug, Default)]
pub struct TurnStatusOutput {
    /// Hit area for the cancel / hard-stop button, if rendered.
    /// `None` when the button is not shown (idle without subagents, cancelling,
    /// drain-blocked, keyboard-only).
    pub cancel_button: Option<Rect>,
    /// Hit area for the global pause / resume button, if rendered.
    pub pause_button: Option<Rect>,
    /// Hit area for the background-demote button, if rendered.
    pub bg_button: Option<Rect>,
    /// Hit area for the still-running watcher cue (click opens the tasks
    /// pane). `None` on keyboard-only hosts.
    pub watching_cue: Option<Rect>,
}

/// Hover state for the turn-status row's mouse affordances (`[stop]`,
/// `[pause]`/`[resume]`, `[↓]`, the still-running watcher cue). `Some(_)`
/// renders them; `None` marks a keyboard-only host (minimal mode — no mouse
/// capture) and suppresses all.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseButtons {
    /// Whether the mouse is over the `[stop]` cancel button.
    pub cancel_hovered: bool,
    /// Whether the mouse is over the `[pause]` / `[resume]` button.
    pub pause_hovered: bool,
    /// Whether the mouse is over the `[↓]` send-to-background button.
    pub bg_hovered: bool,
    /// Whether the mouse is over the still-running watcher cue.
    pub watching_hovered: bool,
}

/// Discoverable work-control hit targets for the turn-status row.
///
/// Named contract (Work B):
/// - **Pause** (quiet white on hover) is global pause (`ToggleGlobalPause`),
///   never hard cancel.
/// - **Stop** (red on hover) is hard cancel (`CancelTurn` / stop-subagents),
///   never global pause.
/// - Hit targets when the primary turn is live, background subagents run, or
///   global pause is active (so resume stays discoverable). Soft stop stays
///   keyboard-only (no button).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkControlChrome {
    pub show_pause: bool,
    pub show_stop: bool,
    /// When true, the pause control is labeled `[resume]` (global pause active).
    pub pause_is_resume: bool,
}

/// Resolve which pause/stop buttons should paint for the current work state.
pub fn work_control_chrome(
    show_buttons: bool,
    turn_running: bool,
    subagents: usize,
    global_paused: bool,
) -> WorkControlChrome {
    if !show_buttons {
        return WorkControlChrome::default();
    }
    let work_live = turn_running || subagents > 0;
    WorkControlChrome {
        show_pause: work_live || global_paused,
        show_stop: work_live,
        pause_is_resume: global_paused,
    }
}

/// Label for the pause/resume control (`leading_space` when a neighbor sits
/// immediately to the left).
fn pause_button_str(is_resume: bool, leading_space: bool) -> &'static str {
    match (is_resume, leading_space) {
        (false, true) => " [pause]",
        (false, false) => "[pause]",
        (true, true) => " [resume]",
        (true, false) => "[resume]",
    }
}

/// Label for the hard-stop control.
fn stop_button_str(leading_space: bool) -> &'static str {
    if leading_space { " [stop]" } else { "[stop]" }
}

/// Counts of idle-surviving "watcher" work — background jobs that can wake
/// the agent for a new turn while it sits idle (commands and monitors on
/// completion/events, `/loop` tasks on a timer, background subagents on
/// finish). They share one persistent still-running cue above the prompt.
/// Broader than the tasks-pane `Watchers` group (monitors + loops only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Watchers {
    /// Running background commands (non-monitor `background: true` tasks).
    pub commands: usize,
    /// Running `monitor` background tasks.
    pub monitors: usize,
    /// Active scheduled `/loop` tasks.
    pub loops: usize,
    /// Running background subagents. While the agent is idle, any running
    /// subagent is a background one — a foreground subagent would keep the
    /// parent in `TurnRunning`.
    pub subagents: usize,
    pub workflows: usize,
}

impl Watchers {
    /// Total watcher count across all kinds.
    pub fn total(self) -> usize {
        self.commands + self.monitors + self.loops + self.subagents + self.workflows
    }

    /// Awaitable in-flight work — the kinds a blocking `wait_tasks` /
    /// `get_task_output` wait can resolve on (commands, monitors, subagents;
    /// scheduled `/loop` tasks and workflows are not task waits).
    pub fn awaitable_work(self) -> usize {
        self.commands + self.monitors + self.subagents
    }
}

/// Format a counts-first `"… still running"` cue from `(count, noun)` pairs,
/// listing only the non-zero kinds (plain-`s` plurals) — e.g.
/// `"1 command · 2 monitors still running"`. `None` when every count is
/// zero. Single owner of the format mechanics so the agent view's idle cue
/// and the dashboard's background-work label cannot drift.
pub(crate) fn format_still_running<'a>(
    kinds: impl IntoIterator<Item = (usize, &'a str)>,
) -> Option<String> {
    use std::fmt::Write as _;
    let mut label = String::with_capacity(48);
    for (count, noun) in kinds {
        if count == 0 {
            continue;
        }
        if !label.is_empty() {
            label.push_str(" \u{00b7} ");
        }
        let plural = if count == 1 { "" } else { "s" };
        let _ = write!(label, "{count} {noun}{plural}");
    }
    if label.is_empty() {
        return None;
    }
    label.push_str(" still running");
    Some(label)
}

/// The idle watcher cue's label — e.g.
/// `"1 command · 2 monitors · 1 loop · 1 subagent still running"`. Leads
/// with the counts (not an ambient "watching") so a glance under a
/// "Worked for X" marker still reads as unfinished work. `None` when no
/// watchers are live.
fn still_running_label(watchers: Watchers) -> Option<String> {
    format_still_running([
        (watchers.commands, "command"),
        (watchers.monitors, "monitor"),
        (watchers.loops, "loop"),
        (watchers.subagents, "subagent"),
        (watchers.workflows, "workflow"),
    ])
}

/// Whether the turn is blocked in a wait the shell aborts as soon as the
/// user sends a message (`get_task_output` with `timeout_ms`, `wait_tasks`,
/// `Await*`, and a foreground subagent await — mirrors the shell's blocking
/// waits, whose send-now routing cancels the blocked turn and runs the new
/// message next). Typing is actionable during these, which is what the
/// parked-wait rendering (`AgentView::is_parked_on_sendable_wait` /
/// `renders_parked`) builds on.
///
/// `Subagent` is included: the shell treats a blocked foreground subagent
/// await like the other blocking waits, so Enter sends promptly and pre-wait
/// rows read as held. `Model` waits stay excluded — the model is actively
/// producing the turn, so a message typed there queues behind real work. Pure
/// predicate over the resolved activity; no turn-lifecycle side effects.
pub fn is_sendable_wait(activity: &Option<TurnActivity>) -> bool {
    matches!(
        activity,
        Some(TurnActivity::Waiting(
            WaitingReason::TaskOutput { waits: true, .. }
                | WaitingReason::TasksComplete
                | WaitingReason::Sleep
                | WaitingReason::Subagent
        ))
    )
}

/// Inputs to [`render_turn_status`] — one frame's worth of turn state.
#[derive(Debug)]
pub struct TurnStatusArgs<'a> {
    pub state: &'a AgentState,
    pub activity: &'a Option<TurnActivity>,
    pub turn_elapsed: Option<Duration>,
    pub activity_started_at: Option<Instant>,
    pub tick: u64,
    pub drain_blocked: bool,
    /// Mouse affordances + hover state; `None` for keyboard-only hosts.
    pub buttons: Option<MouseButtons>,
    pub has_running_execute: bool,
    /// Context-window tokens used, shown as `⇣Nk`.
    pub total_tokens: Option<u64>,
    pub mcp_init_progress: Option<&'a McpInitProgress>,
    pub is_bash_turn: bool,
    pub is_pending_user_input: bool,
    pub goal_verifying: bool,
    pub watchers: Watchers,
    /// Parked on a sendable wait (`AgentView::renders_parked`): suppress the
    /// running-turn chrome and render only the still-running cue.
    pub parked: bool,
    /// Transparent right-side background so the row blends with the
    /// terminal's own background (minimal mode).
    pub flat_background: bool,
    pub held_queue: usize,
    pub held_queue_top_sendable: bool,
    /// Process-level fearless global pause is active (status row keeps a
    /// resume hit target even when every session is idle).
    pub global_paused: bool,
}

/// Render the turn status line into the given area.
///
/// The caller is responsible for only allocating a 1-row area when
/// `should_show()` returns true (and 0 rows when false).
pub fn render_turn_status(
    buf: &mut Buffer,
    area: Rect,
    args: TurnStatusArgs<'_>,
) -> TurnStatusOutput {
    let TurnStatusArgs {
        state,
        activity,
        turn_elapsed,
        activity_started_at,
        tick,
        drain_blocked,
        buttons,
        has_running_execute,
        total_tokens,
        mcp_init_progress,
        is_bash_turn,
        is_pending_user_input,
        goal_verifying,
        watchers,
        parked,
        flat_background,
        held_queue,
        held_queue_top_sendable,
        global_paused,
    } = args;
    // Resolve the mouse affordances: a keyboard-only host (`None`) suppresses
    // clickable buttons and reports no hover.
    let show_buttons = buttons.is_some();
    let cancel_hovered = buttons.is_some_and(|b| b.cancel_hovered);
    let pause_hovered = buttons.is_some_and(|b| b.pause_hovered);
    let bg_hovered = buttons.is_some_and(|b| b.bg_hovered);
    if area.height == 0 || area.width < 10 {
        return TurnStatusOutput::default();
    }

    let theme = Theme::current();
    // Right-side timer/button background. Flat hosts (minimal) use the
    // terminal's own background so the row stays transparent.
    let timer_bg = if flat_background {
        Color::Reset
    } else {
        theme.bg_base
    };
    let right_style = |fg| {
        Style::default()
            .fg(fg)
            .bg(timer_bg)
            .remove_modifier(Modifier::all())
    };

    // MCP startup seed (total == 0) while idle — show "Starting session…"
    // above the prompt until the shell reports real server counts. Real MCP
    // progress (total > 0) renders as the compact top-bar chip instead, not
    // here. Auto-expires via `is_visible()` if the shell never reports.
    if state.is_idle()
        && !drain_blocked
        && let Some(progress) = mcp_init_progress
        && progress.total == 0
        && progress.is_visible()
    {
        render_starting_session(buf, area, progress, tick, &theme);
        return TurnStatusOutput::default();
    }

    // Special case: drain is blocked (user editing front prompt, agent idle).
    // No cancel button in this state.
    if drain_blocked && state.is_idle() {
        // Pulsing diamond in accent_user, blending toward bg.
        let diamond_color = pending_diamond_color(&theme, theme.accent_user, tick);
        let spans = vec![
            Span::styled(
                format!("{} ", crate::glyphs::diamond_filled()),
                Style::default().fg(diamond_color),
            ),
            Span::styled(
                "agent idle ~ waiting on your edit",
                Style::default().fg(theme.gray),
            ),
        ];
        buf.set_line(area.x, area.y, &Line::from(spans), area.width);
        return TurnStatusOutput::default();
    }

    // Idle or parked with watchers: persistent still-running cue (not
    // scrollback — it must never scroll away). Lower priority than the
    // starting-session and drain-blocked cues above.
    //
    // When background subagents hold the pending-prompt queue, append a
    // suffix so the operator sees that Enter queues rather than starting a
    // turn: "Enter queues" with an empty hold, or "N queued — Interject to
    // force" once rows are held. Sendable-wait holds on the running-turn
    // path use "Enter to interject" instead.
    //
    // Work B: when primary is idle (not parked) and subagents are live, also
    // paint discoverable `[pause]` + `[stop]` on the right. Parked still
    // suppresses those (wait aborts on type; stop chrome would lie).
    if (state.is_idle() || parked)
        && let Some(cue) = still_running_label(watchers)
    {
        // Pulsing concentric circle (○ ◎ ◉ ◎) on a calm ambient cadence:
        // the agent is idle, so this breath runs slower than the active
        // turn spinner (see MONITOR_PULSE_DIVISOR). Icon uses accent_running
        // (agent activity / subagent throbber; magenta under DOGE), not
        // accent_system cyan (limits / path / system tags).
        let frames = crate::glyphs::monitor_icon_frames();
        let frame_idx = (tick / MONITOR_PULSE_DIVISOR) as usize % frames.len();
        let icon = format!("{} ", frames[frame_idx]);
        let label_fg = if buttons.is_some_and(|b| b.watching_hovered) {
            theme.text_primary
        } else {
            theme.gray
        };
        // Held-queue suffix, or a pre-queue cue while background subagents hold
        // drain (Enter queues even before the first follow-up is queued).
        // Monitors/commands alone do not hold; only subagents do.
        let queue_suffix = if held_queue > 0 && state.is_idle() {
            if held_queue_top_sendable {
                format!(" · {held_queue} queued — Interject to force")
            } else {
                format!(" · {held_queue} queued")
            }
        } else if state.is_idle() && watchers.subagents > 0 {
            " · Enter queues".to_string()
        } else {
            String::new()
        };
        // Parked: no pause/stop. Idle: subagents (or global pause) unlock them.
        let chrome = if parked {
            WorkControlChrome::default()
        } else {
            work_control_chrome(show_buttons, false, watchers.subagents, global_paused)
        };
        let pause_str = if chrome.show_pause {
            pause_button_str(chrome.pause_is_resume, false)
        } else {
            ""
        };
        let stop_str = if chrome.show_stop {
            stop_button_str(chrome.show_pause)
        } else {
            ""
        };
        // Leading space before the first right control so it does not abut
        // the cue text.
        let right_gap = if chrome.show_pause || chrome.show_stop {
            " "
        } else {
            ""
        };
        let right_width = right_gap.width() + pause_str.width() + stop_str.width();
        let left_budget = (area.width as usize).saturating_sub(right_width);
        let full_label = format!("{cue}{queue_suffix}");
        let cue_width = (icon.width() + full_label.width()).min(left_budget) as u16;
        let mut spans = vec![
            Span::styled(icon, Style::default().fg(theme.accent_running)),
            Span::styled(cue, Style::default().fg(label_fg)),
        ];
        if !queue_suffix.is_empty() {
            spans.push(Span::styled(queue_suffix, Style::default().fg(theme.gray)));
        }
        buf.set_line(area.x, area.y, &Line::from(spans), left_budget as u16);

        let mut x = area.x + area.width.saturating_sub(right_width as u16);
        let mut pause_button = None;
        let mut cancel_button = None;
        if !right_gap.is_empty() {
            let gap_span = Span::styled(right_gap, right_style(theme.gray));
            buf.set_span(x, area.y, &gap_span, right_gap.width() as u16);
            x += right_gap.width() as u16;
        }
        if chrome.show_pause && !pause_str.is_empty() {
            let pause_x = x;
            let pause_fg = if pause_hovered {
                theme.text_primary
            } else {
                theme.gray
            };
            let span = Span::styled(pause_str, right_style(pause_fg));
            buf.set_span(x, area.y, &span, pause_str.width() as u16);
            x += pause_str.width() as u16;
            pause_button = Some(Rect::new(pause_x, area.y, pause_str.width() as u16, 1));
        }
        if chrome.show_stop && !stop_str.is_empty() {
            let stop_x = x;
            let stop_fg = if cancel_hovered {
                theme.accent_error
            } else {
                theme.gray
            };
            let span = Span::styled(stop_str, right_style(stop_fg));
            buf.set_span(x, area.y, &span, stop_str.width() as u16);
            cancel_button = Some(Rect::new(stop_x, area.y, stop_str.width() as u16, 1));
        }

        return TurnStatusOutput {
            watching_cue: show_buttons.then(|| Rect::new(area.x, area.y, cue_width, 1)),
            pause_button,
            cancel_button,
            ..TurnStatusOutput::default()
        };
    }

    // Global pause with nothing else to show: keep a resume hit target so
    // pause is not toast-only after every session goes idle.
    if global_paused && state.is_idle() && !parked {
        let chrome = work_control_chrome(show_buttons, false, 0, true);
        let pause_str = if chrome.show_pause {
            pause_button_str(true, true)
        } else {
            ""
        };
        let right_width = pause_str.width();
        let left_budget = (area.width as usize).saturating_sub(right_width);
        let label = "Paused all work";
        let spans = vec![Span::styled(
            truncate_str(label, left_budget),
            Style::default().fg(theme.gray),
        )];
        buf.set_line(area.x, area.y, &Line::from(spans), left_budget as u16);
        let pause_button = if chrome.show_pause && !pause_str.is_empty() {
            let pause_x = area.x + area.width.saturating_sub(right_width as u16);
            let pause_fg = if pause_hovered {
                theme.text_primary
            } else {
                theme.gray
            };
            let span = Span::styled(pause_str, right_style(pause_fg));
            buf.set_span(pause_x, area.y, &span, right_width as u16);
            Some(Rect::new(pause_x, area.y, right_width as u16, 1))
        } else {
            None
        };
        return TurnStatusOutput {
            pause_button,
            ..TurnStatusOutput::default()
        };
    }

    // Parked with no watchers left: render nothing. The stopped look must
    // never fall through to the running-turn chrome (spinner/timers/[stop])
    // — the wait aborts the moment the user types, so that chrome would lie.
    if parked {
        return TurnStatusOutput::default();
    }

    // Running-turn work-control chrome (pause + hard stop). Cancelling hides
    // both (already stopping). Keyboard-only hosts suppress clickable hits.
    let turn_running_for_buttons = matches!(
        state,
        AgentState::TurnRunning | AgentState::CommandRunning { .. }
    );
    let chrome = work_control_chrome(
        show_buttons,
        turn_running_for_buttons,
        watchers.subagents,
        global_paused,
    );
    let show_cancel = chrome.show_stop;
    let show_pause = chrome.show_pause;

    // ── Compute activity style and label ──
    let (activity_style, label, is_tool) =
        compute_activity(&theme, state, activity, is_bash_turn, goal_verifying);

    // Early return for idle (shouldn't happen if should_show is respected, but be safe).
    if matches!(state, AgentState::Idle) {
        return TurnStatusOutput::default();
    }

    // ── Build right-aligned content first (to know how much space is left) ──
    // Format: `1m20s` or `1m20s ⇣12k` (with tokens).
    let turn_timer_str = match (turn_elapsed, total_tokens) {
        (Some(d), Some(tokens)) if tokens > 0 => {
            format!(
                "{} {}{}",
                format_turn_timer(d),
                crate::glyphs::token_arrow(),
                format_tokens_short(tokens)
            )
        }
        (Some(d), _) => format_turn_timer(d),
        _ => String::new(),
    };
    let turn_timer_width = turn_timer_str.width();

    // Bg button: [↓] normally, [send to bg] when hovered (only for running execute
    // tools). Requires a mouse host with stop chrome available for this turn.
    let show_bg = show_cancel && has_running_execute;
    let bg_str = if show_bg {
        if bg_hovered {
            " [send to bg]"
        } else {
            " [\u{2193}]"
        }
    } else {
        ""
    };
    let bg_width = bg_str.width();

    // Pause (quiet white on hover) then stop (red on hover). Labels are
    // `'static` so the per-frame status line never allocates. Leading space
    // when a neighbor sits left so controls do not fuse.
    let pause_str: &str = if show_pause {
        pause_button_str(chrome.pause_is_resume, show_bg || turn_timer_width > 0)
    } else {
        ""
    };
    let pause_width = pause_str.width();

    // Cancel button: always `[stop]`. Leading space when a neighbor sits left.
    let cancel_str: &str = if show_cancel {
        stop_button_str(show_bg || show_pause || turn_timer_width > 0)
    } else {
        ""
    };
    let cancel_width = cancel_str.width();

    let right_width = turn_timer_width + bg_width + pause_width + cancel_width;

    // ── Build components ──
    // While a tool is blocked on a permission prompt or `ask_user_question`,
    // swap the running braille spinner for a pulsing `◆`. Same animation
    // shape the drain-blocked and plan-approval indicators already use,
    // so every "your turn" status reads with one consistent visual cue.
    let spinner_str = if is_pending_user_input {
        format!("{} ", crate::glyphs::diamond_filled())
    } else {
        let frames = crate::glyphs::braille_spinner_frames();
        let frame_idx = (tick / SPINNER_DIVISOR) as usize % frames.len();
        format!("{} ", frames[frame_idx])
    };
    let spinner_width = spinner_str.width();

    // "Ask" tools (AskUserQuestion): suppress the phase timer so the user
    // doesn't feel time-pressured while answering questions.
    let is_asking = is_tool
        && matches!(
            activity,
            Some(TurnActivity::ToolRunning { title, .. })
                if title.starts_with("Ask: ") || title.starts_with("Ask ")
        );

    // Phase timer (gray, same as turn timer) — hidden for ask tools
    let phase_timer_str = if is_asking {
        String::new()
    } else {
        activity_started_at
            .map(|t| format!(" {}", format_turn_timer(t.elapsed())))
            .unwrap_or_default()
    };
    let phase_timer_width = phase_timer_str.width();

    // Timer style (gray for both phase and turn timers).
    //
    // Right-side elements (turn timer, bg / pause / stop buttons) must set
    // fg, bg, AND remove_modifier explicitly. fill_background() paints
    // bg_base on every cell before widgets render, but set_line() for the
    // left content may overwrite fg/modifiers on cells in the right zone.
    // A Style with bg:None (the default) cannot restore bg after a reset,
    // and a Style without remove_modifier cannot clear leaked modifiers.
    // `timer_bg` / `right_style` are resolved at the top of this function.
    let timer_style = Style::default()
        .fg(theme.gray)
        .bg(timer_bg)
        .remove_modifier(Modifier::all());

    // Available width for activity label (only the label truncates)
    // Layout: spinner + label + phase_timer + queued_hint + gap(1) + turn_timer + cancel
    let min_gap = 1;
    let available_for_label = (area.width as usize)
        .saturating_sub(spinner_width)
        .saturating_sub(phase_timer_width)
        .saturating_sub(min_gap)
        .saturating_sub(right_width);

    // ── Render left side: spinner + label (truncated) + phase_timer + queued_hint ──
    let mut left_spans: Vec<Span<'static>> = Vec::with_capacity(5);

    // Spinner color: usually inherits the activity color (accent_running for
    // tools, secondary for thinking/responding, yellow for retries). While the
    // tool is parked on the user we render `◆` with a smooth pulse from
    // dim→bright in `accent_user`, matching the drain-blocked and
    // plan-approval indicators so every "your turn" status has the same
    // visual cadence.
    let spinner_style = if is_pending_user_input {
        let diamond_color = pending_diamond_color(&theme, theme.accent_user, tick);
        Style::default().fg(diamond_color)
    } else {
        activity_style
    };
    left_spans.push(Span::styled(spinner_str, spinner_style));

    // Activity label (potentially truncated)
    let mut queued_hint: Option<Span<'static>> = None;
    if is_tool {
        if let Some(TurnActivity::ToolRunning { title, description }) = activity {
            if is_asking {
                // Ask tools: render as a unified gray label (like Thinking/Responding),
                // not as a command invocation — yellow is reserved for shell commands.
                let detail = title
                    .strip_prefix("Ask: ")
                    .or_else(|| title.strip_prefix("Ask "))
                    .unwrap_or(title.as_str());
                let msg = format!("Waiting on answers for {detail}");
                let display = truncate_str(&msg, available_for_label);
                left_spans.push(Span::styled(display, activity_style));
            } else if let Some(desc) = description
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                // Bash (and similar) tools carry a human description — prefer
                // that over the raw command for the status line so a sleep /
                // long-running exec reads as `{description}…` rather than
                // `Run sleep 5 && …`.
                let msg = crate::acp::tracker::format_waiting_for_subject(desc);
                let display = truncate_str(&msg, available_for_label);
                left_spans.push(Span::styled(display, activity_style));
            } else if let Some(query) = title.strip_prefix("Web search: ") {
                // Web search: "Search " (muted) + query (yellow)
                let prefix = "Search ";
                let prefix_width = prefix.width();
                let query = query.trim_matches('"');
                let max_query = available_for_label.saturating_sub(prefix_width).max(5);
                let display = truncate_str(query, max_query);
                left_spans.push(Span::styled(prefix, Style::default().fg(theme.gray)));
                left_spans.push(Span::styled(display, Style::default().fg(theme.command)));
            } else if let Some(url) = title.strip_prefix("Fetch: ") {
                // Fetch tools: "Fetch " (muted) + URL (yellow)
                let prefix = "Fetch ";
                let prefix_width = prefix.width();
                let max_url = available_for_label.saturating_sub(prefix_width).max(5);
                let display = truncate_str(url, max_url);
                left_spans.push(Span::styled(prefix, Style::default().fg(theme.gray)));
                left_spans.push(Span::styled(display, Style::default().fg(theme.command)));
            } else {
                // Normal tool: "Run " (muted) + command (syntax-highlighted).
                // For qualified MCP tool names the activity title is the
                // raw `server__action` string from ACP; prettify it to
                // `(Server) Action` so the spinner doesn't show the ugly
                // delimiter form. Non-MCP titles (bash commands etc.) are
                // returned untouched by `mcp_pretty_name_if_qualified`.
                let prefix = "Run ";
                let pretty = mcp_pretty_name_if_qualified(title.as_str());
                let detail = pretty.as_str();
                let prefix_width = prefix.width();
                let max_cmd = available_for_label.saturating_sub(prefix_width).max(5);
                let first_line = detail.lines().next().unwrap_or(detail);
                let display = truncate_str(first_line, max_cmd);
                left_spans.push(Span::styled(prefix, Style::default().fg(theme.gray)));
                left_spans.extend(crate::views::tasks_pane::highlight_bash_command(&display));
            }
        }
    } else {
        // Sendable wait holding queued messages: the persistent inline hint
        // saying why the queue is paused and how to send anyway. On the status
        // row (not an ephemeral tip) so it stays visible for the whole wait,
        // and dropped before the label truncates on a narrow terminal.
        // "Enter to interject" is advertised only when Enter would actually
        // soft-interject the top row (bash / client-expanded local rows refuse
        // with a toast — see `AgentView::held_queue_top_sendable`).
        let suffix = if held_queue > 0 && is_sendable_wait(activity) {
            if held_queue_top_sendable {
                format!(" · {held_queue} queued — Enter to interject")
            } else {
                format!(" · {held_queue} queued")
            }
        } else {
            String::new()
        };
        if !suffix.is_empty() && label.width() + suffix.width() <= available_for_label {
            left_spans.push(Span::styled(label.clone(), activity_style));
            queued_hint = Some(Span::styled(suffix, Style::default().fg(theme.gray)));
        } else {
            let display = truncate_str(&label, available_for_label);
            left_spans.push(Span::styled(display, activity_style));
        }
    }

    // Phase timer (gray, never truncates)
    if !phase_timer_str.is_empty() {
        left_spans.push(Span::styled(phase_timer_str, timer_style));
    }

    // After the phase timer, so the elapsed time reads as the wait's, not the hint's.
    if let Some(hint) = queued_hint {
        left_spans.push(hint);
    }

    // Render left side
    let left_line = Line::from(left_spans);
    buf.set_line(area.x, area.y, &left_line, area.width);

    // ── Render right side: turn_timer + bg + pause + stop ──
    let right_start_x = area.x + area.width.saturating_sub(right_width as u16);

    // Turn timer (gray)
    let mut x = right_start_x;
    if !turn_timer_str.is_empty() {
        let span = Span::styled(turn_timer_str.clone(), timer_style);
        buf.set_span(x, area.y, &span, turn_timer_width as u16);
        x += turn_timer_width as u16;
    }

    // Bg button — accent_running on hover
    let bg_button_rect = if show_bg && !bg_str.is_empty() {
        let bg_x = x;
        let bg_style = if bg_hovered {
            right_style(theme.accent_running)
        } else {
            right_style(theme.gray)
        };
        let span = Span::styled(bg_str, bg_style);
        buf.set_span(x, area.y, &span, bg_width as u16);
        x += bg_width as u16;
        Some(Rect::new(bg_x, area.y, bg_str.width() as u16, 1))
    } else {
        None
    };

    // Pause / resume — quiet white (`text_primary`) on hover, gray at rest.
    // Never uses accent_error; that token is reserved for hard stop.
    let pause_button_rect = if show_pause && !pause_str.is_empty() {
        let pause_x = x;
        let pause_style = if pause_hovered {
            right_style(theme.text_primary)
        } else {
            right_style(theme.gray)
        };
        let span = Span::styled(pause_str, pause_style);
        buf.set_span(x, area.y, &span, pause_width as u16);
        x += pause_width as u16;
        Some(Rect::new(pause_x, area.y, pause_width as u16, 1))
    } else {
        None
    };

    // Cancel / hard stop — accent_error (red) on hover, gray at rest
    let cancel_button_rect = if show_cancel && !cancel_str.is_empty() {
        let cancel_x = x;
        let cancel_style = if cancel_hovered {
            right_style(theme.accent_error)
        } else {
            right_style(theme.gray)
        };
        let span = Span::styled(cancel_str, cancel_style);
        buf.set_span(x, area.y, &span, cancel_width as u16);
        Some(Rect::new(cancel_x, area.y, cancel_width as u16, 1))
    } else {
        None
    };

    TurnStatusOutput {
        cancel_button: cancel_button_rect,
        pause_button: pause_button_rect,
        bg_button: bg_button_rect,
        watching_cue: None,
    }
}

/// Compute activity style, label, and whether it's a tool.
fn compute_activity(
    theme: &Theme,
    state: &AgentState,
    activity: &Option<TurnActivity>,
    is_bash_turn: bool,
    goal_verifying: bool,
) -> (Style, String, bool) {
    match (state, activity) {
        (AgentState::TurnCancelling | AgentState::CommandCancelling { .. }, _) => (
            Style::default().fg(theme.accent_error),
            "Cancelling…".to_string(),
            false,
        ),
        // Goal-mode completion verification runs in-turn after the model
        // stops streaming. The harness drives the skeptic panel (the model
        // itself is idle), but the turn's last streaming activity can still
        // read as `Responding`/`Thinking`; label the whole window
        // "Verifying…" so the multi-minute panel isn't mislabelled as the
        // model responding (or a hung "Waiting…").
        (AgentState::TurnRunning, _) if goal_verifying => (
            Style::default().fg(theme.text_secondary),
            "Verifying…".to_string(),
            false,
        ),
        (AgentState::TurnRunning, Some(TurnActivity::Thinking)) => (
            Style::default().fg(theme.text_secondary),
            "Thinking…".to_string(),
            false,
        ),
        (AgentState::TurnRunning, Some(TurnActivity::Responding)) => (
            Style::default().fg(theme.text_secondary),
            "Responding…".to_string(),
            false,
        ),
        (AgentState::TurnRunning, Some(TurnActivity::ToolRunning { title, description })) => {
            // "Ask" tools (AskUserQuestion) use gray spinner like Thinking —
            // running green/success feels out of place when the user is answering.
            // Human descriptions (e.g. bash `description`) also use muted
            // secondary — they read as a wait subject (`Wait 5s…`), not a
            // running `Run <command>` invocation.
            // Busy tool chrome (spinner + bare Run title) uses accent_running
            // (agent activity), not accent_success (skills/success green).
            let is_ask = title.starts_with("Ask: ") || title.starts_with("Ask ");
            let has_desc = description
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty());
            let style = if is_ask || has_desc {
                Style::default().fg(theme.text_secondary)
            } else {
                Style::default().fg(theme.accent_running)
            };
            (style, String::new(), true)
        }
        (AgentState::TurnRunning, Some(TurnActivity::AutoCompacting)) => (
            Style::default().fg(theme.text_secondary),
            "Compacting…".to_string(),
            false,
        ),
        (
            AgentState::TurnRunning,
            Some(TurnActivity::Retrying {
                attempt,
                max_retries,
                reason,
            }),
        ) => {
            let label = format_retrying_activity_label(*attempt, *max_retries, reason);
            (Style::default().fg(theme.warning), label, false)
        }
        (AgentState::TurnRunning, Some(TurnActivity::Waiting(reason))) => (
            // Explicit wait reason (model / subagent / task output / tasks /
            // sleep): name what the agent is blocked on instead of a generic
            // "Waiting…". See `WaitingReason` and `AgentView::resolve_turn_activity`.
            Style::default().fg(theme.text_secondary),
            reason.label(),
            false,
        ),
        (AgentState::TurnRunning, None) if is_bash_turn => (
            // Bash turn: not inference, show generic "Running…".
            Style::default().fg(theme.text_secondary),
            "Running…".to_string(),
            false,
        ),
        (AgentState::TurnRunning, None) => (
            // Fallback: a running inference turn with no resolved activity. The
            // view resolves this gap into Waiting(Model/Subagent) before render,
            // so this is now a rarely-hit safety net.
            Style::default().fg(theme.text_secondary),
            "Waiting…".to_string(),
            false,
        ),
        (
            AgentState::CommandRunning {
                command:
                    command @ (AgentCommand::CreateWorktree
                    | AgentCommand::RestoreWorktree
                    | AgentCommand::RestoreCode
                    | AgentCommand::ForkSession),
                ..
            },
            _,
        ) => (
            Style::default().fg(theme.gray),
            format!("{}…", command.display_name()),
            false,
        ),
        (AgentState::CommandRunning { command, .. }, _) => (
            Style::default().fg(theme.text_secondary),
            format!("{}…", command.display_name()),
            false,
        ),
        (AgentState::Idle, _) => (Style::default(), String::new(), false),
    }
}

/// Whether the idle "Starting session…" indicator wants the turn-status row.
///
/// True only for a fresh `total == 0` startup seed (gated by
/// [`McpInitProgress::is_visible`] so an orphaned seed expires). Real MCP
/// progress (`total > 0`) renders as the top-bar chip instead, so it does not
/// drive this row.
fn starting_session_visible(progress: Option<&McpInitProgress>) -> bool {
    progress.is_some_and(|p| p.total == 0 && p.is_visible())
}

/// Render the idle "Starting session…" indicator above the prompt.
///
/// Format: `⠋ Starting session… 0:01` — braille spinner + label + elapsed
/// timer. Rendered in `theme.gray_dim` (the dimmest gray) so it reads as
/// quiet/ambient, matching the top-bar MCP chip and the directory path — this
/// is non-blocking startup, not foreground activity. Shown only while the MCP
/// init progress is a startup seed (`total == 0`), before the shell reports
/// real server counts; real progress (`total > 0`) renders as the top-bar chip.
fn render_starting_session(
    buf: &mut Buffer,
    area: Rect,
    progress: &McpInitProgress,
    tick: u64,
    theme: &Theme,
) {
    let frames = crate::glyphs::braille_spinner_frames();
    let frame_idx = (tick / SPINNER_DIVISOR) as usize % frames.len();
    let timer_str = format!(" {}", format_turn_timer(progress.started_at.elapsed()));
    let style = Style::default().fg(theme.gray_dim);
    let spans = vec![
        Span::styled(format!("{} ", frames[frame_idx]), style),
        Span::styled("Starting session…", style),
        Span::styled(timer_str, style),
    ];
    buf.set_line(area.x, area.y, &Line::from(spans), area.width);
}

/// Whether the turn status line should be visible.
///
/// Returns true when a turn is active (Running or Cancelling), when the drain
/// is blocked (agent idle, waiting on user edit), while the MCP startup seed
/// is showing "Starting session…" (a fresh `total == 0` seed), or when the
/// agent is idle but background watchers are still running
/// (`watchers.total() > 0`) — running commands and monitors wake the agent on
/// completion/events, scheduled `/loop` tasks fire prompts, and background
/// subagents inject a completion turn, any of which can start a new turn.
///
/// A parked turn (`parked` — the stopped look while blocked on a sendable
/// wait) suppresses the running-turn chrome entirely: the row shows only when
/// watchers exist, rendering the "… still running" cue.
///
/// Real MCP progress (`total > 0`) renders as a compact chip in the top status
/// bar instead, so it does not affect this row.
pub fn should_show(
    state: &AgentState,
    drain_blocked: bool,
    mcp_init_progress: Option<&McpInitProgress>,
    watchers: Watchers,
    parked: bool,
    global_paused: bool,
) -> bool {
    // Resume must stay discoverable after every session goes idle under pause.
    if global_paused && !parked {
        return true;
    }
    if parked {
        return watchers.total() > 0;
    }
    !state.is_idle()
        || drain_blocked
        || starting_session_visible(mcp_init_progress)
        || watchers.total() > 0
}

/// Shared Retrying chrome for main turn status **and** nested/subagent
/// activity labels (`format_activity_label`). Unlimited budget shows
/// `attempt N`; finite shows `N/M`. Reason uses a middle-dot separator and
/// trailing ellipsis — never the old `Retrying (#N): raw error` form.
pub(crate) fn format_retrying_activity_label(
    attempt: u32,
    max_retries: u32,
    reason: &str,
) -> String {
    // Unlimited budget (u32::MAX) shows attempt only; finite shows N/M.
    let mut label = if max_retries == u32::MAX {
        format!("Retrying (attempt {attempt})")
    } else {
        format!("Retrying ({attempt}/{max_retries})")
    };
    let brief = reason.trim();
    if !brief.is_empty() {
        // Keep status line readable: first line, prefer meaningful
        // transport detail over long reqwest/eventsource prefixes that
        // used to clip to bare "Transport error: error".
        let one_line = brief.lines().next().unwrap_or(brief);
        let clipped = clip_retry_reason_brief(one_line);
        label.push_str(" · ");
        label.push_str(&clipped);
    }
    label.push('…');
    label
}

/// Clip a retry reason for the status footer (~45 visible chars).
///
/// Long `reqwest error stream: Transport error: error sending request…`
/// strings used to clip right after the word `error`, leaving opaque
/// `Transport error: error`. Strip known outer templates and keep the
/// meaningful tail (or a short human label when that is all that remains).
pub(crate) fn clip_retry_reason_brief(one_line: &str) -> String {
    const MAX: usize = 45;
    let s = one_line.trim();
    if s.is_empty() {
        return String::new();
    }
    // Already-short human labels from the sampler (preferred).
    if s.chars().count() <= 48 {
        return s.to_string();
    }
    let mut rest = s;
    for prefix in [
        "reqwest error stream: ",
        "request error: ",
        "Transport error: ",
    ] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped.trim_start();
        }
    }
    // Second pass: eventsource still wraps after stripping the SamplingError prefix.
    if let Some(stripped) = rest.strip_prefix("Transport error: ") {
        rest = stripped.trim_start();
    }
    if rest.is_empty() {
        return "connection interrupted".to_string();
    }
    if rest.chars().count() <= MAX {
        return rest.to_string();
    }
    let t: String = rest.chars().take(MAX).collect();
    // Avoid stranding a lone trailing "error" word from "error sending…".
    if t == "error" || t.ends_with(" error") {
        return "connection interrupted".to_string();
    }
    format!("{t}…")
}

/// Format a duration for the turn/phase timer.
///
/// Re-exports [`crate::util::format_duration`] under the old name for
/// backwards compatibility within this module.
pub use crate::util::format_duration as format_turn_timer;

/// Format a token count for compact display.
///
/// - Under 1000: `1`, `10`, `100` (raw number)
/// - 1k-100k: `1.23k`, `10.1k` (with decimal)
/// - 100k-1m: `100k`, `500k` (whole thousands)
/// - 1m+: `1.23m`, `10.1m` (with decimal)
fn format_tokens_short(tokens: u64) -> String {
    if tokens < 1000 {
        format!("{tokens}")
    } else if tokens < 100_000 {
        // 1k-99.9k: show one or two decimals for precision
        let k = tokens as f64 / 1000.0;
        if tokens < 10_000 {
            format!("{k:.2}k") // 1.23k
        } else {
            format!("{k:.1}k") // 10.1k
        }
    } else if tokens < 1_000_000 {
        // 100k-999k: whole thousands
        let k = tokens / 1000;
        format!("{k}k")
    } else {
        // 1m+: show with decimal
        let m = tokens as f64 / 1_000_000.0;
        if tokens < 10_000_000 {
            format!("{m:.2}m") // 1.23m
        } else {
            format!("{m:.1}m") // 10.1m
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Sendable waits = exactly the wait reasons the shell aborts on a queued
    /// user prompt (blocking task-output / wait_tasks / Await, and a blocked
    /// foreground subagent await — all take the send-now path). Model waits —
    /// where typing only queues behind the actively-streaming turn — and
    /// non-wait activities keep the busy spinner.
    #[test]
    fn sendable_wait_matches_shell_interruptible_waits() {
        let task_wait = |waits| {
            Some(TurnActivity::Waiting(WaitingReason::TaskOutput {
                task_ids: vec!["t-1".into()],
                subject: Some("sleep 300".into()),
                waits,
            }))
        };
        assert!(is_sendable_wait(&task_wait(true)));
        assert!(
            !is_sendable_wait(&task_wait(false)),
            "instant polls are not blocking waits"
        );
        assert!(is_sendable_wait(&Some(TurnActivity::Waiting(
            WaitingReason::TasksComplete
        ))));
        assert!(is_sendable_wait(&Some(TurnActivity::Waiting(
            WaitingReason::Sleep
        ))));
        assert!(!is_sendable_wait(&Some(TurnActivity::Waiting(
            WaitingReason::Model
        ))));
        assert!(
            is_sendable_wait(&Some(TurnActivity::Waiting(WaitingReason::Subagent))),
            "the shell aborts a blocked foreground subagent await on send-now, \
             so Enter during it must read as sendable"
        );
        assert!(!is_sendable_wait(&Some(TurnActivity::Thinking)));
        assert!(!is_sendable_wait(&None));
    }

    #[test]
    fn format_subsecond() {
        assert_eq!(format_turn_timer(Duration::from_millis(500)), "0.5s");
        assert_eq!(format_turn_timer(Duration::from_millis(120)), "0.1s");
    }

    /// Contract: long transport Display templates must not clip to bare
    /// `Transport error: error` in the footer reason slot.
    #[test]
    fn clip_retry_reason_does_not_strand_bare_error_word() {
        let long = "reqwest error stream: Transport error: error sending request for url (https://api.x.ai/v1/chat/completions)";
        let clipped = clip_retry_reason_brief(long);
        assert!(
            !clipped.eq_ignore_ascii_case("error")
                && !clipped.ends_with("Transport error: error")
                && !clipped.ends_with("Transport error: error…"),
            "clip must not strand bare 'error', got {clipped:?}"
        );
        assert!(
            clipped.contains("sending request") || clipped == "connection interrupted",
            "expected meaningful transport detail or short label, got {clipped:?}"
        );
    }

    #[test]
    fn clip_retry_reason_keeps_short_human_label() {
        assert_eq!(
            clip_retry_reason_brief("connection interrupted"),
            "connection interrupted"
        );
    }

    #[test]
    fn retrying_activity_label_uses_clipped_reason() {
        let theme = Theme::current();
        let activity = Some(TurnActivity::Retrying {
            attempt: 1,
            max_retries: u32::MAX,
            reason: "connection interrupted".into(),
        });
        let (_, label, _) =
            compute_activity(&theme, &AgentState::TurnRunning, &activity, false, false);
        assert!(
            label.starts_with("Retrying (attempt 1) · connection interrupted"),
            "got {label:?}"
        );
    }

    /// Contract: network recovery status is plain (timed out / reconnecting /
    /// N of M when budget is finite), never zombie "Waiting for response…".
    #[test]
    fn retrying_label_shows_timeout_backoff_and_reconnecting() {
        let theme = Theme::current();
        let timed_out = Some(TurnActivity::Retrying {
            attempt: 1,
            max_retries: u32::MAX,
            reason: "timed out · next try in 2s".into(),
        });
        let (_, label, _) =
            compute_activity(&theme, &AgentState::TurnRunning, &timed_out, false, false);
        assert!(
            label.starts_with("Retrying (attempt 1) · timed out · next try in 2s"),
            "got {label:?}"
        );
        assert!(
            !label.contains("Waiting for response"),
            "retry chrome must not look like a zombie wait, got {label:?}"
        );

        let finite = Some(TurnActivity::Retrying {
            attempt: 2,
            max_retries: 5,
            reason: "connection interrupted · next try in 4s".into(),
        });
        let (_, label, _) =
            compute_activity(&theme, &AgentState::TurnRunning, &finite, false, false);
        assert!(
            label.starts_with("Retrying (2/5) · connection interrupted · next try in 4s"),
            "finite budget must show N/M, got {label:?}"
        );

        let reconnecting = Some(TurnActivity::Retrying {
            attempt: 1,
            max_retries: u32::MAX,
            reason: "reconnecting".into(),
        });
        let (_, label, _) = compute_activity(
            &theme,
            &AgentState::TurnRunning,
            &reconnecting,
            false,
            false,
        );
        assert!(
            label.starts_with("Retrying (attempt 1) · reconnecting"),
            "post-StreamResumed soft reconnect, got {label:?}"
        );
    }

    #[test]
    fn format_under_10s_has_decimal() {
        assert_eq!(format_turn_timer(Duration::from_secs_f64(5.2)), "5.2s");
        assert_eq!(format_turn_timer(Duration::from_secs_f64(9.9)), "9.9s");
    }

    #[test]
    fn format_10s_plus_no_decimal() {
        assert_eq!(format_turn_timer(Duration::from_secs(10)), "10s");
        assert_eq!(format_turn_timer(Duration::from_secs(32)), "32s");
        assert_eq!(format_turn_timer(Duration::from_secs(59)), "59s");
    }

    #[test]
    fn format_minutes() {
        assert_eq!(format_turn_timer(Duration::from_secs(60)), "1m0s");
        assert_eq!(format_turn_timer(Duration::from_secs(80)), "1m20s");
        assert_eq!(format_turn_timer(Duration::from_secs(600)), "10m0s");
    }

    #[test]
    fn activity_label_reads_verifying_while_goal_verifying_overriding_stale_activity() {
        let theme = Theme::current();
        // Running turn, no streaming activity, goal verifying → "Verifying…".
        let (_, label, _) = compute_activity(&theme, &AgentState::TurnRunning, &None, false, true);
        assert_eq!(label, "Verifying…");
        // Same state without the verifying flag → generic "Waiting…".
        let (_, label, _) = compute_activity(&theme, &AgentState::TurnRunning, &None, false, false);
        assert_eq!(label, "Waiting…");
        // During verification the model is idle but its last streaming
        // activity (Responding/Thinking) can linger — the flag overrides it
        // so the panel reads "Verifying…", not "Responding…" (the bug).
        for activity in [TurnActivity::Responding, TurnActivity::Thinking] {
            let (_, label, _) = compute_activity(
                &theme,
                &AgentState::TurnRunning,
                &Some(activity),
                false,
                true,
            );
            assert_eq!(label, "Verifying…");
        }
        // Without the flag the streaming label stands.
        let (_, label, _) = compute_activity(
            &theme,
            &AgentState::TurnRunning,
            &Some(TurnActivity::Responding),
            false,
            false,
        );
        assert_eq!(label, "Responding…");
    }

    #[test]
    fn waiting_reason_renders_specific_label() {
        use crate::acp::tracker::WaitingReason;
        let theme = Theme::current();
        let cases = [
            (WaitingReason::Model, "Waiting for response…"),
            (WaitingReason::Subagent, "Waiting on subagent…"),
            (WaitingReason::task_output(), "Waiting on task output…"),
            (
                WaitingReason::TaskOutput {
                    task_ids: vec!["t1".into()],
                    subject: Some("compile release".into()),
                    waits: false,
                },
                "compile release…",
            ),
            (WaitingReason::TasksComplete, "Waiting on tasks…"),
            (WaitingReason::Sleep, "Sleeping…"),
        ];
        for (reason, expected) in cases {
            let (_, label, is_tool) = compute_activity(
                &theme,
                &AgentState::TurnRunning,
                &Some(TurnActivity::Waiting(reason.clone())),
                false,
                false,
            );
            assert_eq!(label, expected, "reason {reason:?}");
            assert!(!is_tool, "waiting is not a tool activity");
        }
    }

    #[test]
    fn bash_turn_still_renders_running_not_waiting() {
        let theme = Theme::current();
        // A bash (non-inference) turn with no activity keeps its own "Running…"
        // label — the view leaves it as `None` rather than Waiting(Model).
        let (_, label, _) = compute_activity(&theme, &AgentState::TurnRunning, &None, true, false);
        assert_eq!(label, "Running…");
    }

    /// Lower-left tool/activity throbber uses `accent_running`, not success
    /// green — under DOGE that is pure magenta (#FF00FF). Skills keep
    /// `accent_skill` green separately.
    #[test]
    fn doge_tool_running_spinner_uses_accent_running_not_success_green() {
        let doge = Theme::doge();
        let magenta = ratatui::style::Color::Rgb(255, 0, 255);
        let green = ratatui::style::Color::Rgb(0, 255, 0);
        assert_eq!(doge.accent_running, magenta);
        assert_eq!(
            doge.accent_success, green,
            "success/skills family stays green"
        );

        let tool = TurnActivity::ToolRunning {
            title: "Bash".into(),
            description: None,
        };
        let (style, _, is_tool) =
            compute_activity(&doge, &AgentState::TurnRunning, &Some(tool), false, false);
        assert!(is_tool);
        assert_eq!(
            style.fg,
            Some(doge.accent_running),
            "tool activity spinner must use accent_running (magenta under DOGE)"
        );
        assert_ne!(
            style.fg,
            Some(doge.accent_success),
            "must not paint agent throbber with accent_success green"
        );
    }

    /// Shared paint maps to `accent_running`; GrokNight tokens stay as defined
    /// (success still green; running still its own token).
    #[test]
    fn groknight_tool_running_uses_accent_running_tokens_unchanged() {
        let gn = Theme::groknight();
        let tool = TurnActivity::ToolRunning {
            title: "Bash".into(),
            description: None,
        };
        let (style, _, is_tool) =
            compute_activity(&gn, &AgentState::TurnRunning, &Some(tool), false, false);
        assert!(is_tool);
        assert_eq!(style.fg, Some(gn.accent_running));
        // Token inventory: success ≠ running (green skill/success vs running accent).
        assert_ne!(
            gn.accent_success, gn.accent_running,
            "GrokNight success and running tokens must remain distinct"
        );
    }

    #[test]
    fn format_hours() {
        assert_eq!(format_turn_timer(Duration::from_secs(3600)), "1h0m");
        assert_eq!(format_turn_timer(Duration::from_secs(3725)), "1h2m");
    }

    #[test]
    fn should_show_when_running() {
        assert!(should_show(
            &AgentState::TurnRunning,
            false,
            None,
            Watchers::default(),
            false,
            false
        ));
        assert!(should_show(
            &AgentState::TurnCancelling,
            false,
            None,
            Watchers::default(),
            false,
            false
        ));
        assert!(!should_show(
            &AgentState::Idle,
            false,
            None,
            Watchers::default(),
            false,
            false
        ));
    }

    #[test]
    fn should_show_when_drain_blocked() {
        assert!(should_show(
            &AgentState::Idle,
            true,
            None,
            Watchers::default(),
            false,
            false
        ));
    }

    #[test]
    fn should_show_when_watchers_running() {
        // Idle but a watcher (command, monitor, loop, or subagent) is still
        // running → row stays visible so the persistent "… still running" cue
        // can show.
        for watchers in [
            Watchers {
                commands: 1,
                ..Watchers::default()
            },
            Watchers {
                monitors: 1,
                ..Watchers::default()
            },
            Watchers {
                loops: 1,
                ..Watchers::default()
            },
            Watchers {
                subagents: 1,
                ..Watchers::default()
            },
        ] {
            assert!(should_show(
                &AgentState::Idle,
                false,
                None,
                watchers,
                false,
                false
            ));
        }
        // Idle with no watchers and nothing else pending → hidden.
        assert!(!should_show(
            &AgentState::Idle,
            false,
            None,
            Watchers::default(),
            false,
            false
        ));
    }

    #[test]
    fn should_show_parked_only_with_watchers() {
        // Parked (turn running but rendering the stopped look): the row shows
        // only to carry the "… still running" cue — never the running chrome.
        assert!(should_show(
            &AgentState::TurnRunning,
            false,
            None,
            Watchers {
                commands: 1,
                ..Watchers::default()
            },
            true,
            false
        ));
        assert!(!should_show(
            &AgentState::TurnRunning,
            false,
            None,
            Watchers::default(),
            true,
            false
        ));
    }

    #[test]
    fn should_show_when_starting_session() {
        // A fresh total == 0 seed shows "Starting session…" above the prompt.
        let seed = McpInitProgress {
            total: 0,
            connected: 0,
            started_at: Instant::now(),
        };
        assert!(should_show(
            &AgentState::Idle,
            false,
            Some(&seed),
            Watchers::default(),
            false,
            false
        ));

        // Real progress (total > 0) is the top-bar chip — it must NOT drive
        // this row.
        let connecting = McpInitProgress {
            total: 3,
            connected: 1,
            started_at: Instant::now(),
        };
        assert!(!should_show(
            &AgentState::Idle,
            false,
            Some(&connecting),
            Watchers::default(),
            false,
            false
        ));

        // An expired seed must not drive the row either.
        let expired = McpInitProgress {
            total: 0,
            connected: 0,
            started_at: Instant::now() - McpInitProgress::SEED_EXPIRE - Duration::from_secs(1),
        };
        assert!(!should_show(
            &AgentState::Idle,
            false,
            Some(&expired),
            Watchers::default(),
            false,
            false
        ));
    }

    /// Collect every rendered glyph in `area` into a single string.
    fn buffer_text(buf: &Buffer, area: Rect) -> String {
        (area.y..area.y + area.height)
            .map(|y| {
                (area.x..area.x + area.width)
                    .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Baseline render args: idle agent on a mouse host with the given watchers.
    fn idle_args<'a>(watchers: Watchers) -> TurnStatusArgs<'a> {
        TurnStatusArgs {
            state: &AgentState::Idle,
            activity: &None,
            turn_elapsed: None,
            activity_started_at: None,
            tick: 0,
            drain_blocked: false,
            buttons: Some(MouseButtons::default()),
            has_running_execute: false,
            total_tokens: None,
            mcp_init_progress: None,
            is_bash_turn: false,
            is_pending_user_input: false,
            goal_verifying: false,
            watchers,
            parked: false,
            flat_background: false,
            held_queue: 0,
            held_queue_top_sendable: false,
            global_paused: false,
        }
    }

    /// Render `args` into a `width`×1 row.
    fn render_row(args: TurnStatusArgs<'_>, width: u16) -> (TurnStatusOutput, Buffer) {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let output = render_turn_status(&mut buf, area, args);
        (output, buf)
    }

    /// Render `args` into a `width`×1 row, returning the visible text.
    fn render_row_text(args: TurnStatusArgs<'_>, width: u16) -> String {
        let (_, buf) = render_row(args, width);
        buffer_text(&buf, buf.area)
    }

    /// Invoke `render_turn_status` for an idle agent with the given MCP seed.
    fn render_idle_with_mcp(progress: &McpInitProgress) -> String {
        let mut args = idle_args(Watchers::default());
        args.mcp_init_progress = Some(progress);
        render_row_text(args, 60)
    }

    /// Invoke `render_turn_status` for an idle agent with the given watcher
    /// counts at animation tick `tick`.
    fn render_idle_with_watchers_at_tick(watchers: Watchers, tick: u64) -> String {
        render_idle_with_watchers_in_width(watchers, tick, 72)
    }

    /// [`render_idle_with_watchers_at_tick`] with an explicit row width.
    fn render_idle_with_watchers_in_width(watchers: Watchers, tick: u64, width: u16) -> String {
        let mut args = idle_args(watchers);
        args.tick = tick;
        render_row_text(args, width)
    }

    /// Invoke `render_turn_status` for a PARKED running turn (the stopped
    /// look) with the given watcher counts.
    fn render_parked_with_watchers(watchers: Watchers) -> String {
        let activity = Some(TurnActivity::Waiting(WaitingReason::TasksComplete));
        let mut args = idle_args(watchers);
        args.state = &AgentState::TurnRunning;
        args.activity = &activity;
        args.turn_elapsed = Some(Duration::from_secs(5));
        args.parked = true;
        render_row_text(args, 72)
    }

    /// Invoke `render_turn_status` for an idle agent with the given watcher
    /// counts at the first animation tick.
    fn render_idle_with_watchers(watchers: Watchers) -> String {
        render_idle_with_watchers_at_tick(watchers, 0)
    }

    /// Invoke `render_turn_status` for an idle agent with `n` running
    /// monitors at animation tick `tick`.
    fn render_idle_with_monitors_at_tick(n: usize, tick: u64) -> String {
        render_idle_with_watchers_at_tick(
            Watchers {
                monitors: n,
                ..Watchers::default()
            },
            tick,
        )
    }

    /// Invoke `render_turn_status` for an idle agent with `n` running
    /// monitors at the first animation tick.
    fn render_idle_with_monitors(n: usize) -> String {
        render_idle_with_monitors_at_tick(n, 0)
    }

    #[test]
    fn idle_with_monitors_renders_still_running_cue() {
        let text = render_idle_with_monitors(2);
        assert!(
            text.contains("2 monitors still running"),
            "idle with monitors must render the still-running cue, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_one_monitor_uses_singular() {
        let text = render_idle_with_monitors(1);
        assert!(
            text.contains("1 monitor still running") && !text.contains("monitors"),
            "single monitor must use the singular noun, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_no_monitors_renders_nothing() {
        let text = render_idle_with_monitors(0);
        assert!(
            text.trim().is_empty(),
            "idle with no monitors must render nothing, got: {text:?}"
        );
    }

    /// Mouse hosts get a hit rect hugging exactly the rendered cue text, and
    /// hover brightens the label; keyboard-only hosts get neither.
    #[test]
    fn watching_cue_is_clickable_on_mouse_hosts_only() {
        let theme = Theme::current();
        let watchers = Watchers {
            monitors: 1,
            ..Watchers::default()
        };
        // First label cell (after the 2-col icon).
        let label_fg = |buf: &Buffer| buf.cell((2, 0)).map(|c| c.fg);

        let (output, buf) = render_row(idle_args(watchers), 60);
        let rect = output.watching_cue.expect("mouse host must get a hit rect");
        let rendered_width = buffer_text(&buf, buf.area).trim_end().width() as u16;
        assert_eq!(rect, Rect::new(0, 0, rendered_width, 1));
        assert_eq!(label_fg(&buf), Some(theme.gray));

        let mut args = idle_args(watchers);
        args.buttons = Some(MouseButtons {
            watching_hovered: true,
            ..MouseButtons::default()
        });
        let (_, buf) = render_row(args, 60);
        assert_eq!(label_fg(&buf), Some(theme.text_primary));

        let mut args = idle_args(watchers);
        args.buttons = None;
        let (output, _) = render_row(args, 60);
        assert!(output.watching_cue.is_none());
    }

    #[test]
    fn idle_with_loops_renders_still_running_cue() {
        let text = render_idle_with_watchers(Watchers {
            loops: 2,
            ..Watchers::default()
        });
        assert!(
            text.contains("2 loops still running"),
            "idle with loops must render the still-running cue, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_one_loop_uses_singular() {
        let text = render_idle_with_watchers(Watchers {
            loops: 1,
            ..Watchers::default()
        });
        assert!(
            text.contains("1 loop still running") && !text.contains("loops"),
            "single loop must use the singular noun, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_subagents_renders_still_running_cue() {
        let text = render_idle_with_watchers(Watchers {
            subagents: 2,
            ..Watchers::default()
        });
        assert!(
            text.contains("2 subagents still running"),
            "idle with subagents must render the still-running cue, got: {text:?}"
        );
    }

    /// Lower-left still-running throbber (○ ◎ ◉ beside "N subagents still
    /// running") is agent activity chrome: `accent_running` (magenta under
    /// DOGE), not `accent_system` cyan used for limits / path / system tags.
    #[test]
    fn doge_idle_subagent_still_running_throbber_uses_accent_running_not_system_cyan() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let doge = Theme::doge();
        let magenta = Color::Rgb(255, 0, 255);
        let cyan = Color::Rgb(0, 255, 255);
        assert_eq!(
            doge.accent_running, magenta,
            "DOGE accent_running is pure magenta"
        );
        assert_eq!(doge.accent_system, cyan, "DOGE accent_system is pure cyan");
        assert_ne!(doge.accent_running, doge.accent_system);

        let watchers = Watchers {
            subagents: 2,
            ..Watchers::default()
        };
        let (_, buf) = render_row(idle_args(watchers), 60);
        let icon_fg = buf.cell((0, 0)).map(|c| c.fg);
        assert_eq!(
            icon_fg,
            Some(doge.accent_running),
            "still-running throbber must paint accent_running (magenta under DOGE), got {icon_fg:?}"
        );
        assert_ne!(
            icon_fg,
            Some(doge.accent_system),
            "still-running throbber must not use accent_system cyan"
        );
        let text = buffer_text(&buf, buf.area);
        assert!(
            text.contains("2 subagents still running"),
            "sanity: cue text present, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_subagents_and_held_queue_shows_force_hint() {
        let mut args = idle_args(Watchers {
            subagents: 1,
            ..Watchers::default()
        });
        args.held_queue = 1;
        args.held_queue_top_sendable = true;
        let text = render_row_text(args, 90);
        assert!(
            text.contains("1 subagent still running")
                && text.contains("1 queued — Interject to force"),
            "idle background hold must explain the queue + how to force, got: {text:?}"
        );
        assert!(
            !text.contains("Enter queues"),
            "force hint replaces the empty-queue Enter queues cue; got: {text:?}"
        );
    }

    /// Named contract: idle + live background subagent(s) with nothing queued
    /// yet still tell the operator that Enter will queue (not start a turn).
    #[test]
    fn idle_with_subagents_empty_queue_shows_enter_queues_cue() {
        let text = render_idle_with_watchers(Watchers {
            subagents: 1,
            ..Watchers::default()
        });
        assert!(
            text.contains("1 subagent still running") && text.contains("Enter queues"),
            "idle hold with empty queue must advertise that Enter queues, got: {text:?}"
        );
        assert!(
            !text.contains("Interject to force"),
            "empty queue must not show force-drain suffix yet, got: {text:?}"
        );
    }

    /// Monitors alone do not hold the queue; do not claim Enter queues.
    #[test]
    fn idle_with_monitors_only_does_not_show_enter_queues_cue() {
        let text = render_idle_with_watchers(Watchers {
            monitors: 1,
            ..Watchers::default()
        });
        assert!(
            text.contains("1 monitor still running"),
            "sanity: monitor cue present, got: {text:?}"
        );
        assert!(
            !text.contains("Enter queues"),
            "monitors must not hold the queue or claim Enter queues, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_one_subagent_uses_singular() {
        let text = render_idle_with_watchers(Watchers {
            subagents: 1,
            ..Watchers::default()
        });
        assert!(
            text.contains("1 subagent still running") && !text.contains("subagents"),
            "single subagent must use the singular noun, got: {text:?}"
        );
        // Same state also carries the empty-queue Enter cue (see dedicated test).
        assert!(
            text.contains("Enter queues"),
            "singular subagent idle hold still needs Enter queues cue, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_one_workflow_counts_run_once() {
        let text = render_idle_with_watchers(Watchers {
            workflows: 1,
            ..Watchers::default()
        });
        assert!(text.contains("1 workflow still running"), "got: {text:?}");
    }

    #[test]
    fn idle_with_monitors_and_loops_lists_both() {
        // Both watcher kinds present → one cue lists monitors then loops,
        // each with its own count, joined by the middle-dot separator.
        let text = render_idle_with_watchers(Watchers {
            monitors: 1,
            loops: 2,
            ..Watchers::default()
        });
        assert!(
            text.contains("1 monitor \u{00b7} 2 loops still running"),
            "both kinds must be listed in one cue, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_all_watcher_kinds_lists_all() {
        // Commands, monitors, loops, and subagents present → one cue lists
        // all four in order, middle-dot separated. Wide row so pause/stop
        // chrome does not clip the leading counts.
        let text = render_idle_with_watchers_in_width(
            Watchers {
                commands: 1,
                monitors: 2,
                loops: 1,
                subagents: 3,
                workflows: 0,
            },
            0,
            100,
        );
        assert!(
            text.contains(
                "1 command \u{00b7} 2 monitors \u{00b7} 1 loop \u{00b7} 3 subagents still running"
            ),
            "all kinds must be listed in one cue, got: {text:?}"
        );
    }

    #[test]
    fn narrow_area_clips_cue_tail_keeping_counts() {
        // 40 cols with three kinds: the row tail-clips with no ellipsis, so
        // the leading counts survive and the trailing suffix is what gets
        // cut. Pins the narrow-pane tradeoff of leading with the counts; a
        // smarter compact fallback would be a behavior change.
        let watchers = Watchers {
            commands: 1,
            monitors: 2,
            loops: 1,
            ..Watchers::default()
        };
        let text = render_idle_with_watchers_in_width(watchers, 0, 40);
        assert!(
            text.contains("1 command \u{00b7} 2 monitors \u{00b7} 1 loop"),
            "the counts must survive the clip, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_commands_renders_still_running_cue() {
        // Plain background commands (non-monitor bg tasks) count as watchers:
        // they wake the agent with a task-completed turn, so the cue must show.
        let text = render_idle_with_watchers(Watchers {
            commands: 2,
            ..Watchers::default()
        });
        assert!(
            text.contains("2 commands still running"),
            "idle with bg commands must render the still-running cue, got: {text:?}"
        );
        let text = render_idle_with_watchers(Watchers {
            commands: 1,
            ..Watchers::default()
        });
        assert!(
            text.contains("1 command still running") && !text.contains("commands"),
            "single command must use the singular noun, got: {text:?}"
        );
    }

    #[test]
    fn parked_with_watchers_renders_cue_not_running_chrome() {
        // A parked running turn renders the still-running cue — never the busy
        // spinner/timers/[stop] chrome (the wait aborts as soon as the user
        // types, so that chrome would lie).
        let text = render_parked_with_watchers(Watchers {
            commands: 2,
            ..Watchers::default()
        });
        assert!(
            text.contains("2 commands still running"),
            "parked with bg work must render the still-running cue, got: {text:?}"
        );
        assert!(
            !text.contains("Waiting") && !text.contains("[stop]"),
            "parked must not render the running-turn chrome, got: {text:?}"
        );
    }

    #[test]
    fn parked_without_watchers_renders_nothing() {
        let text = render_parked_with_watchers(Watchers::default());
        assert!(
            text.trim().is_empty(),
            "parked with no watchers must render nothing, got: {text:?}"
        );
    }

    #[test]
    fn idle_with_no_watchers_renders_nothing() {
        let text = render_idle_with_watchers(Watchers::default());
        assert!(
            text.trim().is_empty(),
            "idle with no watchers must render nothing, got: {text:?}"
        );
    }

    #[test]
    fn queued_hint_renders_after_phase_timer() {
        let activity = Some(TurnActivity::Waiting(WaitingReason::Subagent));
        let mut args = idle_args(Watchers::default());
        args.state = &AgentState::TurnRunning;
        args.activity = &activity;
        args.activity_started_at = Some(Instant::now() - Duration::from_secs(359));
        args.held_queue = 1;
        args.held_queue_top_sendable = true;
        let text = render_row_text(args, 80);
        assert!(
            text.contains("Waiting on subagent… 5m59s · 1 queued — Enter to interject"),
            "phase timer must sit between the wait label and the queued hint, got: {text:?}"
        );
    }

    #[test]
    fn still_running_label_lists_only_nonzero_kinds() {
        assert_eq!(
            still_running_label(Watchers {
                commands: 2,
                ..Watchers::default()
            }),
            Some("2 commands still running".into())
        );
        assert_eq!(
            still_running_label(Watchers {
                monitors: 2,
                ..Watchers::default()
            }),
            Some("2 monitors still running".into())
        );
        assert_eq!(
            still_running_label(Watchers {
                loops: 1,
                ..Watchers::default()
            }),
            Some("1 loop still running".into())
        );
        assert_eq!(
            still_running_label(Watchers {
                subagents: 1,
                ..Watchers::default()
            }),
            Some("1 subagent still running".into())
        );
        assert_eq!(
            still_running_label(Watchers {
                monitors: 1,
                loops: 2,
                ..Watchers::default()
            }),
            Some("1 monitor \u{00b7} 2 loops still running".into())
        );
        assert_eq!(
            still_running_label(Watchers {
                commands: 1,
                monitors: 1,
                loops: 1,
                subagents: 2,
                workflows: 0,
            }),
            Some(
                "1 command \u{00b7} 1 monitor \u{00b7} 1 loop \u{00b7} 2 subagents still running"
                    .into()
            )
        );
        assert_eq!(still_running_label(Watchers::default()), None);
    }

    #[test]
    fn idle_monitor_icon_animates_across_ticks() {
        // The leading glyph cycles through monitor_icon_frames() as `tick`
        // advances, so two ticks a full frame apart (0 vs MONITOR_PULSE_DIVISOR)
        // must render different icons — proving the cue is animated, not static.
        let frame0 = render_idle_with_monitors_at_tick(1, 0);
        let frame1 = render_idle_with_monitors_at_tick(1, MONITOR_PULSE_DIVISOR);
        let icon0 = frame0.chars().next();
        let icon1 = frame1.chars().next();
        assert_ne!(
            icon0, icon1,
            "monitor icon must animate between frames, got {frame0:?} vs {frame1:?}"
        );
    }

    #[test]
    fn idle_zero_server_seed_renders_starting_session() {
        // total == 0 seed → "Starting session…" above the prompt.
        let text = render_idle_with_mcp(&McpInitProgress {
            total: 0,
            connected: 0,
            started_at: Instant::now(),
        });
        assert!(
            text.contains("Starting session"),
            "idle 0-server seed must render 'Starting session…', got: {text:?}"
        );
    }

    #[test]
    fn idle_active_mcp_progress_renders_nothing_in_turn_status() {
        // total > 0 is the top-bar chip — the turn-status row stays empty.
        let text = render_idle_with_mcp(&McpInitProgress {
            total: 3,
            connected: 1,
            started_at: Instant::now(),
        });
        assert!(
            text.trim().is_empty(),
            "active MCP progress must NOT render in the turn-status row, got: {text:?}"
        );
    }

    #[test]
    fn expired_seed_renders_nothing() {
        // An expired total == 0 seed renders nothing — defense-in-depth.
        let text = render_idle_with_mcp(&McpInitProgress {
            total: 0,
            connected: 0,
            started_at: Instant::now() - McpInitProgress::SEED_EXPIRE - Duration::from_secs(1),
        });
        assert!(
            text.trim().is_empty(),
            "expired seed must render nothing, got: {text:?}"
        );
    }

    #[test]
    fn format_tokens_under_1k() {
        assert_eq!(format_tokens_short(0), "0");
        assert_eq!(format_tokens_short(1), "1");
        assert_eq!(format_tokens_short(10), "10");
        assert_eq!(format_tokens_short(100), "100");
        assert_eq!(format_tokens_short(999), "999");
    }

    #[test]
    fn format_tokens_1k_to_10k() {
        assert_eq!(format_tokens_short(1000), "1.00k");
        assert_eq!(format_tokens_short(1230), "1.23k");
        assert_eq!(format_tokens_short(1500), "1.50k");
        assert_eq!(format_tokens_short(9990), "9.99k");
        assert_eq!(format_tokens_short(9999), "10.00k"); // rounds up
    }

    #[test]
    fn format_tokens_10k_to_100k() {
        assert_eq!(format_tokens_short(10000), "10.0k");
        assert_eq!(format_tokens_short(10100), "10.1k");
        assert_eq!(format_tokens_short(12345), "12.3k");
        assert_eq!(format_tokens_short(99999), "100.0k"); // rounds up
    }

    #[test]
    fn format_tokens_100k_to_1m() {
        assert_eq!(format_tokens_short(100000), "100k");
        assert_eq!(format_tokens_short(128000), "128k");
        assert_eq!(format_tokens_short(500000), "500k");
        assert_eq!(format_tokens_short(999000), "999k");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens_short(1_000_000), "1.00m");
        assert_eq!(format_tokens_short(1_230_000), "1.23m");
        assert_eq!(format_tokens_short(9_999_000), "10.00m"); // rounds
        assert_eq!(format_tokens_short(10_000_000), "10.0m");
        assert_eq!(format_tokens_short(10_100_000), "10.1m");
    }

    #[test]
    fn user_waiting_pulse_speed_is_stable() {
        // The drain-blocked, pending-user-input, and plan-approval cues
        // all read from this single constant via `pending_diamond_color`,
        // so this assertion guards against an accidental tweak that
        // would silently change the cadence of every "your turn" cue.
        assert_eq!(USER_WAITING_PULSE_SPEED, 0.08);
    }

    /// DOGE waiting diamond must solid-step between pure primaries — never
    /// mid-channel gray from `blend_color` alpha fade.
    #[test]
    fn doge_pending_diamond_color_stays_on_pure_palette_no_gray_blend() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = Theme::doge();
        let accent = theme.accent_user; // pure green
        let is_pure = |c: Color| -> bool {
            matches!(
                c,
                Color::Rgb(0, 0, 0)
                    | Color::Rgb(255, 0, 0)
                    | Color::Rgb(0, 255, 0)
                    | Color::Rgb(255, 255, 0)
                    | Color::Rgb(0, 0, 255)
                    | Color::Rgb(255, 0, 255)
                    | Color::Rgb(0, 255, 255)
                    | Color::Rgb(255, 255, 255)
            )
        };
        let mut saw_accent = false;
        let mut saw_black = false;
        for tick in 0..200u64 {
            let c = pending_diamond_color(&theme, accent, tick);
            assert!(
                is_pure(c),
                "tick {tick}: diamond color {c:?} must be a DOGE pure primary (no gray blend)"
            );
            if c == accent {
                saw_accent = true;
            }
            if c == theme.bg_base {
                saw_black = true;
            }
            // Reject equal-channel mid grays explicitly.
            if let Color::Rgb(r, g, b) = c {
                assert!(
                    !(r == g && g == b && r > 0 && r < 255),
                    "tick {tick}: mid-gray RGB({r},{g},{b}) forbidden under DOGE"
                );
            }
        }
        assert!(saw_accent, "cycle must hit full accent");
        assert!(saw_black, "cycle must hit pure black trough (solid step)");
    }

    // ── Work B: pause (white) vs stop (red) discoverability ──────────────

    /// Named contract: pause paints when work is live or global pause is on;
    /// stop paints only when a primary turn or subagents can be cancelled.
    #[test]
    fn work_control_chrome_matrix_pause_not_cancel_stop_not_pause() {
        // Keyboard-only: no hits.
        assert_eq!(
            work_control_chrome(false, true, 2, false),
            WorkControlChrome::default()
        );
        // Mid-turn: both; pause is not resume.
        assert_eq!(
            work_control_chrome(true, true, 0, false),
            WorkControlChrome {
                show_pause: true,
                show_stop: true,
                pause_is_resume: false,
            }
        );
        // Idle primary + live subagents: both (discoverable stop path).
        assert_eq!(
            work_control_chrome(true, false, 1, false),
            WorkControlChrome {
                show_pause: true,
                show_stop: true,
                pause_is_resume: false,
            }
        );
        // Idle, no subagents: nothing (monitors alone do not unlock stop).
        assert_eq!(
            work_control_chrome(true, false, 0, false),
            WorkControlChrome::default()
        );
        // Global pause with idle sessions: resume only (no stop).
        assert_eq!(
            work_control_chrome(true, false, 0, true),
            WorkControlChrome {
                show_pause: true,
                show_stop: false,
                pause_is_resume: true,
            }
        );
        // Global pause mid-work: resume + stop still available.
        assert_eq!(
            work_control_chrome(true, true, 0, true),
            WorkControlChrome {
                show_pause: true,
                show_stop: true,
                pause_is_resume: true,
            }
        );
    }

    /// Mid-turn mouse host paints both `[pause]` and `[stop]`; stop hover is
    /// red, pause hover is quiet white (`text_primary`), never the same token.
    #[test]
    fn mid_turn_paints_pause_and_stop_with_distinct_hover_colors() {
        let _pin = crate::theme::cache::pin_theme();
        crate::theme::cache::set(crate::theme::ThemeKind::Doge);
        let theme = Theme::current();
        assert_ne!(
            theme.text_primary, theme.accent_error,
            "sanity: pause and stop hover tokens must differ"
        );
        let activity = Some(TurnActivity::Thinking);
        let mut args = idle_args(Watchers::default());
        args.state = &AgentState::TurnRunning;
        args.activity = &activity;
        args.turn_elapsed = Some(Duration::from_secs(12));
        args.buttons = Some(MouseButtons::default());
        let (output, buf) = render_row(args, 80);
        let text = buffer_text(&buf, buf.area);
        assert!(
            text.contains("[pause]") && text.contains("[stop]"),
            "mid-turn must paint both controls, got: {text:?}"
        );
        assert!(
            output.pause_button.is_some() && output.cancel_button.is_some(),
            "both hit rects must arm on a mouse host"
        );
        // Pause and stop must be distinct controls (not the same rect).
        assert_ne!(output.pause_button, output.cancel_button);

        // Hover colors: pause → text_primary; stop → accent_error.
        let mut hover = idle_args(Watchers::default());
        hover.state = &AgentState::TurnRunning;
        hover.activity = &activity;
        hover.turn_elapsed = Some(Duration::from_secs(12));
        hover.buttons = Some(MouseButtons {
            pause_hovered: true,
            cancel_hovered: true,
            ..MouseButtons::default()
        });
        let (out, buf) = render_row(hover, 80);
        let pause_rect = out.pause_button.expect("pause hit");
        let stop_rect = out.cancel_button.expect("stop hit");
        // Sample a non-space glyph cell inside each hit rect.
        let cell_fg = |rect: Rect| {
            (rect.x..rect.x + rect.width)
                .find_map(|x| {
                    let c = buf.cell((x, rect.y))?;
                    if c.symbol() != " " { Some(c.fg) } else { None }
                })
                .expect("glyph inside hit rect")
        };
        assert_eq!(
            cell_fg(pause_rect),
            theme.text_primary,
            "pause hover must be quiet white (text_primary)"
        );
        assert_eq!(
            cell_fg(stop_rect),
            theme.accent_error,
            "stop hover must be accent_error red"
        );
    }

    /// Idle primary with live subagents still paints stop (and pause) so cancel
    /// is discoverable without a running parent turn.
    #[test]
    fn idle_with_subagents_paints_pause_and_stop_hits() {
        let watchers = Watchers {
            subagents: 2,
            ..Watchers::default()
        };
        let (output, buf) = render_row(idle_args(watchers), 90);
        let text = buffer_text(&buf, buf.area);
        assert!(
            text.contains("2 subagents still running"),
            "cue must remain, got: {text:?}"
        );
        assert!(
            text.contains("[pause]") && text.contains("[stop]"),
            "idle + subagents must paint pause and stop, got: {text:?}"
        );
        assert!(
            text.contains("Enter queues"),
            "Work A cue must not regress, got: {text:?}"
        );
        assert!(
            output.pause_button.is_some() && output.cancel_button.is_some(),
            "both hit rects required when subagents are live"
        );
    }

    /// Monitors alone do not unlock stop/pause (only primary turn or subagents).
    #[test]
    fn idle_with_monitors_only_does_not_paint_pause_or_stop() {
        let watchers = Watchers {
            monitors: 1,
            ..Watchers::default()
        };
        let (output, buf) = render_row(idle_args(watchers), 80);
        let text = buffer_text(&buf, buf.area);
        assert!(
            text.contains("1 monitor still running"),
            "cue present, got: {text:?}"
        );
        assert!(
            !text.contains("[pause]") && !text.contains("[stop]"),
            "monitors alone must not paint pause/stop, got: {text:?}"
        );
        assert!(output.pause_button.is_none() && output.cancel_button.is_none());
    }

    /// Global pause with idle sessions: row stays visible with `[resume]` only.
    #[test]
    fn global_paused_idle_paints_resume_not_stop() {
        assert!(should_show(
            &AgentState::Idle,
            false,
            None,
            Watchers::default(),
            false,
            true
        ));
        let mut args = idle_args(Watchers::default());
        args.global_paused = true;
        let (output, buf) = render_row(args, 60);
        let text = buffer_text(&buf, buf.area);
        assert!(
            text.contains("Paused all work") && text.contains("[resume]"),
            "global pause idle must paint resume chrome, got: {text:?}"
        );
        assert!(
            !text.contains("[stop]") && !text.contains("[pause]"),
            "resume-only while paused idle, got: {text:?}"
        );
        assert!(output.pause_button.is_some());
        assert!(output.cancel_button.is_none());
    }

    /// Keyboard-only hosts never arm pause/stop hits (chord remains).
    #[test]
    fn keyboard_only_suppresses_pause_and_stop_hits() {
        let activity = Some(TurnActivity::Thinking);
        let mut args = idle_args(Watchers::default());
        args.state = &AgentState::TurnRunning;
        args.activity = &activity;
        args.buttons = None;
        let (output, buf) = render_row(args, 80);
        let text = buffer_text(&buf, buf.area);
        assert!(
            !text.contains("[pause]") && !text.contains("[stop]"),
            "keyboard-only must not paint buttons, got: {text:?}"
        );
        assert!(output.pause_button.is_none() && output.cancel_button.is_none());
    }
}
