//! Fullscreen subagent takeover for [`AgentView`]: opening and closing the child view, drawing its framed transcript,
//! routing input to it, and fetching a child for a live update (which hydrates a resumed child's transcript first).
//!
//! Invariants: `active_subagent` is the sole "takeover open" signal, and every close routes through
//! `close_subagent_fullscreen` so `evict_finished_child_view` runs exactly once. While open, the intercept runs as
//! step 0 of `handle_input_inner`, before any parent routing, and the child's `pending_effects` are hoisted because
//! `AppView` drains only the top-level view's queue. Ctrl+Q is never consumed here; it always bubbles to the global quit.
use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::agent_view::child_action_filter::filter_child_outcome;
use crate::app::agent_view::viewer::IdleEnterQuote;
use crate::app::agent_view::{AgentView, AppRenderParams, OverlayHeader};
use crate::app::app_view::InputOutcome;
use crate::key;
use crate::render::SafeBuf;
use crate::scrollback::render::ScratchBuffer;
use crate::theme::Theme;
use crate::views::agent;
use crate::views::shortcuts_bar::PendingHint;
use crate::views::tasks_pane::TaskEntryId;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
/// What a subagent's fullscreen takeover inherits from a parent that sits in the dashboard overlay. The header and footer
/// keep describing the parent, so its title, switcher, and `Ctrl+X` action come from it (its pending confirmation already
/// arrives as `pending_hint`).
#[derive(Clone, Copy)]
pub(super) struct InheritedOverlay<'a> {
    pub(super) header: OverlayHeader<'a>,
    pub(super) stop_label: &'static str,
}
impl AgentView {
    /// The open takeover, if any. `active_subagent` is the only signal.
    pub(crate) fn visible_nested_overlay_sid(&self) -> Option<&str> {
        self.active_subagent.as_deref()
    }
    /// Open the fullscreen subagent view for `child_sid`, replaying child `updates.jsonl` when the child scrollback is still empty (or the child finished).
    pub(crate) fn open_subagent_fullscreen(&mut self, child_sid: String) {
        let Some(child) = self.subagent_views.get(&child_sid) else {
            return;
        };
        let parent_session_id = child
            .child_link()
            .map(|link| link.parent_session_id().clone());
        if self.active_subagent.as_deref() != Some(child_sid.as_str()) {
            self.close_subagent_fullscreen();
        }
        let replay_outcome = crate::app::subagent::ensure_subagent_child_replayed(self, &child_sid);
        tracing::debug!(
            child_sid = %child_sid,
            ?parent_session_id,
            ?replay_outcome,
            "opened subagent fullscreen"
        );
        let open_l2_floor = self
            .subagent_views
            .get(&child_sid)
            .is_some_and(|child| child.composer_route() == super::ComposerRoute::L2Floor);
        if open_l2_floor && let Some(child) = self.subagent_views.get_mut(&child_sid) {
            let _ = child.set_active_pane(super::AgentPane::Prompt, true);
        }
        self.active_subagent = Some(child_sid);
    }
    /// Open the takeover for the child the selected row links to (`RenderBlock::child_session_id`). `false`, so the
    /// caller keeps its routing, on a group header, an unlinked row, or a child this view does not own (a row
    /// replayed inside a child transcript).
    pub(crate) fn try_open_child_from_selected_row(&mut self) -> bool {
        if self.scrollback.is_selected_group_header() {
            return false;
        }
        let Some(child_sid) = self
            .scrollback
            .selected()
            .and_then(|idx| self.scrollback.entry(idx))
            .and_then(|entry| entry.block.child_session_id())
            .filter(|child_sid| self.subagent_views.contains_key(*child_sid))
            .map(str::to_owned)
        else {
            return false;
        };
        self.open_subagent_fullscreen(child_sid);
        true
    }
    /// Close the fullscreen subagent takeover (if any), evicting the closed child when finished.
    /// See [`crate::app::subagent::evict_finished_child_view`] for rationale and guards.
    /// All close sites route through here.
    pub(crate) fn close_subagent_fullscreen(&mut self) {
        if let Some(return_sid) = self.overlay_return_sid.clone()
            && self.subagent_views.contains_key(&return_sid)
            && self
                .active_subagent
                .as_deref()
                .is_some_and(|active| active != return_sid.as_str())
        {
            if let Some(active_sid) = self.active_subagent.clone() {
                let _ = crate::app::subagent::evict_finished_child_view(self, &active_sid);
            }
            self.overlay_return_sid = None;
            self.active_subagent = Some(return_sid);
            return;
        }
        self.overlay_return_sid = None;
        if let Some(child_sid) = self.active_subagent.take() {
            let _ = crate::app::subagent::evict_finished_child_view(self, &child_sid);
        }
    }

    /// Empty Enter on an L2 floor opens the L3 and remembers the L2.
    /// A selected child row wins. Otherwise the single running specialist
    /// whose parent session is this L2. Does not close or cancel the L2.
    fn open_l3_from_l2(&mut self) -> bool {
        let Some(l2_sid) = self.active_subagent.clone() else {
            return false;
        };
        let is_floor = self
            .subagent_views
            .get(&l2_sid)
            .is_some_and(|view| view.composer_route() == super::ComposerRoute::L2Floor);
        if !is_floor {
            return false;
        }
        let from_row = self.subagent_views.get(&l2_sid).and_then(|l2| {
            l2.scrollback
                .selected()
                .and_then(|idx| l2.scrollback.entry(idx))
                .and_then(|entry| entry.block.child_session_id())
                .map(str::to_owned)
        });
        let target = from_row
            .filter(|sid| sid != &l2_sid && self.subagent_views.contains_key(sid))
            .or_else(|| {
                let mut running: Vec<String> = crate::app::subagent::l3_rows_for_l2(
                    self.subagent_sessions.values(),
                    l2_sid.as_str(),
                )
                .into_iter()
                .filter(|info| {
                    self.subagent_views
                        .contains_key(info.child_session_id.as_ref())
                })
                .map(|info| info.child_session_id.to_string())
                .collect();
                if running.len() == 1 {
                    running.pop()
                } else {
                    None
                }
            });
        let Some(target) = target else {
            return false;
        };
        self.overlay_return_sid = Some(l2_sid);
        self.active_subagent = Some(target);
        true
    }

    /// A painted specialist count on the L1 list opens that running L3.
    /// Esc or q returns to the L2. This does not send KillSubagent.
    pub(crate) fn open_running_l3_from_list(&mut self, l2_sid: &str) -> bool {
        if !self.subagent_views.contains_key(l2_sid) {
            return false;
        }
        let mut running: Vec<String> =
            crate::app::subagent::l3_rows_for_l2(self.subagent_sessions.values(), l2_sid)
                .into_iter()
                .filter(|info| {
                    self.subagent_views
                        .contains_key(info.child_session_id.as_ref())
                })
                .map(|info| info.child_session_id.to_string())
                .collect();
        running.sort();
        let Some(target) = running.into_iter().next() else {
            return false;
        };
        self.overlay_return_sid = Some(l2_sid.to_string());
        self.active_subagent = Some(target);
        true
    }

    /// Floor Enter sends to that L2. Missing session id does not target L1.
    fn l2_floor_send_interject(&mut self, l2_sid: &str, text: String) -> bool {
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            return false;
        }
        let Some(session_id) = self
            .subagent_views
            .get(l2_sid)
            .and_then(|view| view.session.session_id.clone())
        else {
            return false;
        };
        self.pending_effects
            .push(crate::app::actions::Effect::SendInterject {
                agent_id: self.session.id,
                session_id,
                text: trimmed,
                interjection_id: uuid::Uuid::new_v4().to_string(),
                blocks: None,
            });
        if let Some(child) = self.subagent_views.get_mut(l2_sid) {
            child.prompt.set_text("");
        }
        true
    }
    /// Fetch a child view for applying a live update, hydrating a resumed child's inherited transcript first.
    /// The incoming block then never closes the replay window (see [`crate::app::subagent::replay_resumed_child_before_live_block`]).
    /// The funnel for every apply that can be a resumed child's *first* live block: the ACP and xAI child ingresses and the finish-path finalize.
    pub(crate) fn child_view_for_live_update_mut(
        &mut self,
        child_sid: &str,
    ) -> Option<&mut AgentView> {
        crate::app::subagent::replay_resumed_child_before_live_block(self, child_sid);
        self.subagent_views.get_mut(child_sid).map(|v| &mut **v)
    }
    /// The open L3 frame paints that specialist's own context.
    /// Both the used count and the window have to be present and above zero.
    /// A missing figure stays off the frame. It is not printed as 0, and the
    /// L1 sampling window is not copied onto the specialist.
    fn paint_opened_l3_context(&mut self, child_sid: &str) {
        let stamp = {
            let Some(info) = self.subagent_sessions.get(child_sid) else {
                return;
            };
            if info.attempt.depth != Some(2) {
                return;
            }
            match (
                info.attempt.tokens_used.filter(|&count| count > 0),
                info.attempt
                    .context_window_tokens
                    .filter(|&count| count > 0),
            ) {
                (Some(used), Some(window)) => Some((used, window)),
                _ => None,
            }
        };
        let Some(child) = self.subagent_views.get_mut(child_sid) else {
            return;
        };
        if let Some((used, window)) = stamp {
            child.context_state = Some(xai_grok_shell::session::ContextInfo::from_notification(
                used, window,
            ));
            child.session_sampling_window = Some(window);
            child.session.models.override_context_window(window);
            return;
        }
        if child
            .context_state
            .as_ref()
            .is_some_and(|ctx| ctx.used == 0)
        {
            child.context_state = None;
        }
    }

    /// Paint the takeover frame (title row with status, elapsed, and the `[\u{2717}]` close button) around the child view's own `draw`.
    /// Returns the child's cursor and post-flush so the parent frame forwards them unchanged.
    #[expect(clippy::too_many_arguments)]
    pub(super) fn draw_subagent_fullscreen(
        &mut self,
        child_sid: &str,
        area: Rect,
        buf: &mut Buffer,
        registry: &ActionRegistry,
        scratch: &mut ScratchBuffer,
        pending_hint: Option<PendingHint>,
        theme: &Theme,
        overlay: Option<InheritedOverlay<'_>>,
    ) -> (
        Option<(u16, u16)>,
        Option<crate::terminal::overlay::PostFlush>,
    ) {
        use crate::app::subagent::{format_context_badge, format_subagent_label};
        use ratatui::style::Modifier;
        use unicode_width::UnicodeWidthStr;
        self.hit_overlay_nested_status.clear();
        self.overlay_nested_status_child_sid = None;
        self.tasks.view_button_rects.clear();
        let appearance = self.scrollback.appearance().clone();
        let layout_cfg = &appearance.scrollback.layout;
        let compact = appearance.prompt.compact;
        agent::fill_background(buf, area, layout_cfg, compact, theme);
        let padded = Rect {
            x: area.x + layout_cfg.eff_hpad_left(compact),
            y: area.y + layout_cfg.eff_outer_vpad(compact),
            width: area.width.saturating_sub(
                layout_cfg.eff_hpad_left(compact) + layout_cfg.eff_hpad_right(compact),
            ),
            height: area
                .height
                .saturating_sub(layout_cfg.eff_outer_vpad(compact) * 2),
        };
        if padded.width < 10 || padded.height < 5 {
            return (None, crate::terminal::overlay::clear().map(Into::into));
        }
        let border_color = theme.selection_border;
        let frame = match crate::views::picker::render_bordered_frame(
            buf,
            padded,
            border_color,
            theme.bg_base,
        ) {
            Some(f) => f,
            None => {
                return (None, crate::terminal::overlay::clear().map(Into::into));
            }
        };
        let title_y = frame.title_row.y;
        let _title_row = frame.title_row;
        let mut inner = frame.content;
        let _border_style = Style::default().fg(border_color);
        // Copy the row fields before the overlay mirror. `badge` used to borrow the
        // session map through the mutable specialist call below.
        let (
            is_running,
            elapsed,
            type_label,
            description,
            icon,
            icon_color,
            label_color,
            meta,
            badge,
        ) = {
            let info = self.subagent_sessions.get(child_sid);
            let raw_description = info.map(|s| s.description.as_ref()).unwrap_or("subagent");
            let is_running = info.is_some_and(|s| s.is_running());
            let elapsed = info
                .map(|s| crate::util::format_duration(s.display_elapsed()))
                .unwrap_or_default();
            let (type_label, description): (String, String) = match info {
                Some(s) => format_subagent_label(s),
                None => (String::new(), raw_description.to_string()),
            };
            let completed = info.is_some_and(|s| {
                s.status.as_deref() == Some("completed")
                    || s.attempt.status.as_deref() == Some("completed")
            });
            let icon = if is_running {
                let elapsed_ms = info
                    .map(|s| u64::try_from(s.display_elapsed().as_millis()).unwrap_or(u64::MAX))
                    .unwrap_or(0);
                crate::glyphs::sparkler_frame_at_ms(elapsed_ms)
            } else if completed {
                crate::glyphs::check_mark()
            } else {
                crate::glyphs::ballot_x()
            };
            let icon_color = if is_running {
                theme.accent_running
            } else if completed {
                theme.accent_success
            } else {
                theme.accent_error
            };
            let label_color = if info.is_some_and(|s| s.attempt.pending_kill) {
                theme.accent_error
            } else if is_running {
                theme.accent_running
            } else if completed {
                theme.accent_success
            } else {
                theme.accent_error
            };
            let meta = info
                .and_then(|s| s.attempt.model.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("")
                .to_string();
            let badge = info.map(format_context_badge).unwrap_or("").to_string();
            (
                is_running,
                elapsed,
                type_label,
                description,
                icon,
                icon_color,
                label_color,
                meta,
                badge,
            )
        };
        let borrowed_specialists = self.mirror_parented_specialists_for_overlay(child_sid);
        if let Some(child_view) = self.subagent_views.get_mut(child_sid) {
            // Title wait chrome calls resolve_turn_activity on this child.
            // Set the overlay flag before that so L3s are not L1-filtered.
            child_view.tasks.set_overlay_l2_session_id(Some(child_sid));
        }
        let mut activity_label: Option<String> = if is_running {
            self.subagent_views
                .get(child_sid)
                .and_then(|cv| match cv.resolve_turn_activity() {
                    Some(crate::acp::tracker::TurnActivity::WritingToolCall(writing)) => writing
                        .tool_name
                        .clone()
                        .filter(|name| !name.is_empty())
                        .or_else(|| {
                            Some(crate::app::subagent::format_activity_label(
                                &crate::acp::tracker::TurnActivity::WritingToolCall(writing),
                            ))
                        }),
                    Some(activity) => Some(crate::app::subagent::format_activity_label(&activity)),
                    None => cv.session.state.is_busy().then(|| "Waiting".to_string()),
                })
        } else {
            None
        };
        // Unlabeled TaskOutput keeps `Waiting on task output…` on the tracker.
        // This overlay paints `Waiting on 1 task...` when that wait has one
        // nested specialist, including an empty description.
        if activity_label.as_deref() == Some(WAITING_ON_TASK_OUTPUT)
            && overlay_nested_specialist_count(self, child_sid) == 1
        {
            activity_label = Some(WAITING_ON_ONE_TASK.to_string());
        }
        let title_x = padded.x + 1;
        buf.set_span_safe(
            title_x,
            title_y,
            &Span::styled(format!(" {icon}"), Style::default().fg(icon_color)),
            3,
        );
        let close_text = "[\u{2717}]";
        let close_width: u16 = close_text.width() as u16;
        let elapsed_text = elapsed.clone();
        let right_margin: u16 = 1;
        let badge_width = if badge.is_empty() {
            0
        } else {
            badge.width() as u16 + 1
        };
        let activity_width: u16 = activity_label
            .as_deref()
            .map(|s| s.width() as u16 + 3)
            .unwrap_or(0);
        let right_width = activity_width
            + elapsed_text.width() as u16
            + 1
            + close_width
            + right_margin
            + badge_width;
        let desc_start_x = title_x + 3;
        let avail = padded.width.saturating_sub(5 + right_width) as usize;
        let type_text = if type_label.is_empty() {
            String::new()
        } else if description.is_empty() {
            type_label.clone()
        } else {
            format!("{type_label} ")
        };
        let meta_text = if meta.is_empty() {
            String::new()
        } else {
            format!(" {meta}")
        };
        let overhead = type_text.width() + meta_text.width();
        let desc_max = avail.saturating_sub(overhead);
        let desc_display = crate::render::line_utils::truncate_str(&description, desc_max);
        if !type_text.is_empty() {
            buf.set_span_safe(
                desc_start_x,
                title_y,
                &Span::styled(&type_text, Style::default().fg(label_color)),
                type_text.width() as u16,
            );
        }
        let desc_x = desc_start_x + type_text.width() as u16;
        buf.set_span_safe(
            desc_x,
            title_y,
            &Span::styled(
                &desc_display,
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            desc_display.width() as u16,
        );
        let after_desc_x = desc_x + desc_display.width() as u16;
        if !meta_text.is_empty() {
            buf.set_span_safe(
                after_desc_x,
                title_y,
                &Span::styled(&meta_text, Style::default().fg(theme.gray)),
                meta_text.width() as u16,
            );
        }
        let mut rx = padded.x + padded.width.saturating_sub(right_margin + close_width + 1);
        let close_style = if self.hit_subagent_frame_close.hovered {
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.gray)
        };
        buf.set_span_safe(
            rx,
            title_y,
            &Span::styled(close_text, close_style),
            close_width,
        );
        self.hit_subagent_frame_close.rect = Some(Rect::new(rx, title_y, close_width, 1));
        rx = rx.saturating_sub(elapsed_text.width() as u16 + 1);
        buf.set_span_safe(
            rx,
            title_y,
            &Span::styled(&elapsed_text, Style::default().fg(theme.gray)),
            elapsed_text.width() as u16,
        );
        if let Some(activity) = activity_label.as_deref() {
            let segment = format!("{activity} \u{00b7} ");
            let w = segment.width() as u16;
            rx = rx.saturating_sub(w);
            buf.set_span_safe(
                rx,
                title_y,
                &Span::styled(segment, Style::default().fg(theme.gray)),
                w,
            );
        }
        if !badge.is_empty() {
            rx = rx.saturating_sub(badge.width() as u16 + 1);
            buf.set_span_safe(
                rx,
                title_y,
                &Span::styled(badge.as_str(), Style::default().fg(theme.gray_dim)),
                badge.width() as u16,
            );
        }
        let mut child_cursor = None;
        let mut child_post_flush = None;
        self.paint_opened_l3_context(child_sid);
        // One-task empty-description wait: title already paints
        // `Waiting on 1 task...`. Keep a dedicated waiting row strictly
        // below that title so `[↗]` is not collapsed onto the title cell.
        let reserved_wait_row =
            reserved_one_task_wait_row(self, child_sid, activity_label.as_deref(), title_y, inner);
        if reserved_wait_row.is_some() {
            inner.y = inner.y.saturating_add(1);
            inner.height = inner.height.saturating_sub(1);
        }
        if inner.width > 5
            && inner.height > 3
            && let Some(child_view) = self.subagent_views.get_mut(child_sid)
        {
            child_view.tasks.set_overlay_l2_session_id(Some(child_sid));
            // Row clocks only on the L2 window, not every nested draw.
            // A known L3 specialist leaves the flag off. An overlay with no
            // registry row keeps the flag so that window can still show clocks.
            let l2_window = !self.subagent_sessions.contains_key(child_sid)
                || crate::app::subagent::overlay_child_is_l2_coordinator(
                    &self.subagent_sessions,
                    child_sid,
                );
            child_view.activity_row_clocks = l2_window;
            let (cursor, post_flush) = child_view.draw(
                inner,
                buf,
                registry,
                scratch,
                pending_hint,
                false,
                crate::app::agent_view::BannerSlotParams::none(),
                overlay.is_some(),
                false,
                &mut Vec::new(),
                AppRenderParams {
                    overlay_header: overlay.map(|o| o.header).unwrap_or_default(),
                    overlay_stop_label: overlay.map(|o| o.stop_label),
                    ..AppRenderParams::default()
                },
            );
            child_cursor = cursor;
            child_post_flush = post_flush;
        }
        self.record_overlay_specialist_status_hit(buf, area, child_sid, reserved_wait_row);
        self.drop_mirrored_overlay_specialists(child_sid, &borrowed_specialists);
        (child_cursor, child_post_flush)
    }

    /// The specialist status the L2 overlay just painted is a click target.
    /// A left click opens that L3 session view.
    /// `Waiting on 1 task...` wins when that row is painted and the specialist
    /// description is empty. The hit is the lowest row of that phrase, plus
    /// the open-arrow control painted on that row. That waiting row is
    /// strictly below the takeover title: title y and waiting y are not
    /// the same cell. An unlabeled `Waiting on task output…` title and a
    /// bottom `1 subagent still running` cue for this one-task wait are
    /// rewritten to the same phrase so the lower row is the control. When
    /// those cues are missing, a reserved row under the title still paints
    /// the phrase and hosts `[↗]`. Named L3 specialist rows also paint that
    /// same enlarge/open-arrow; activating it opens that L3. Each named row
    /// gets its own arrow and hit. One shared sid is not enough when two
    /// named L3 rows are on screen. A waiting-row-only arrow is not the
    /// L3-row control. Wait chrome that already paints
    /// `{description}: {last tool}` keeps that tool name; the arrow does
    /// not share glyphs with it (`[↗]ad_file` from `read_file`). Live wait
    /// chrome clamps a long description (`{prefix}…: {tool}`); the named
    /// path must still find that painted prefix so a normal-width L2
    /// overlay shows the enlarge/open-arrow. When the specialist list also
    /// paints a dedicated prefix, wait-chrome last-tool still hosts `[↗]`
    /// on the row that names `read_file`, and the named row keeps its own.
    fn record_overlay_specialist_status_hit(
        &mut self,
        buf: &mut Buffer,
        area: Rect,
        child_sid: &str,
        reserved_wait_row: Option<Rect>,
    ) {
        let mut specialists: Vec<(String, String, std::time::Instant)> =
            crate::app::subagent::l3_rows_for_l2(self.subagent_sessions.values(), child_sid)
                .into_iter()
                .map(|info| {
                    (
                        info.child_session_id.to_string(),
                        info.description.trim().to_string(),
                        info.attempt.started_at,
                    )
                })
                .collect();
        specialists.sort_by_key(|(_, _, started)| *started);
        if specialists.len() == 1 {
            paint_one_task_wait_over_phrase(buf, area, WAITING_ON_TASK_OUTPUT);
            if let Some(row) = reserved_wait_row {
                paint_waiting_phrase_on_row(buf, row);
            }
            if painted_phrase_rect(buf, area, WAITING_ON_ONE_TASK).is_some() {
                paint_one_task_wait_over_still_running(buf, area);
                if let Some(mut rect) = lowest_painted_phrase_rect(buf, area, WAITING_ON_ONE_TASK)
                    && let Some((id, phrase, _)) = specialists.first()
                    && phrase.is_empty()
                {
                    if let Some(title) = painted_phrase_rect(buf, area, WAITING_ON_ONE_TASK)
                        && rect.y <= title.y
                    {
                        if let Some(row) = reserved_wait_row.filter(|row| row.y > title.y) {
                            paint_waiting_phrase_on_row(buf, row);
                            if let Some(forced) =
                                painted_phrase_rect_on_row(buf, area, row.y, WAITING_ON_ONE_TASK)
                            {
                                rect = forced;
                            }
                        }
                    }
                    if painted_phrase_rect(buf, area, WAITING_ON_ONE_TASK)
                        .is_some_and(|title| rect.y > title.y)
                    {
                        let hit = paint_waiting_row_open_arrow(buf, area, rect);
                        self.record_overlay_nested_status_hit(id.clone(), hit);
                        return;
                    }
                }
            }
        }
        for (id, phrase, _) in specialists {
            if phrase.is_empty() {
                continue;
            }
            let mut rects = Vec::new();
            // Wait-chrome `{prefix}…: {tool}` wins for overlay hit paint when
            // the specialist list also painted a dedicated prefix. The named
            // test looks at the `read_file` wait-chrome row, not the list.
            if let Some(rect) = painted_last_tool_wait_chrome_phrase_rect(buf, area, &phrase) {
                rects.push(rect);
            }
            if let Some(rect) = painted_specialist_phrase_rect(buf, area, &phrase)
                && !rects.iter().any(|existing| existing.y == rect.y)
            {
                rects.push(rect);
            }
            for rect in rects {
                let hit = paint_waiting_row_open_arrow(buf, area, rect);
                self.record_overlay_nested_status_hit(id.clone(), hit);
            }
        }
    }

    /// One named L3 row, or the empty-description one-task wait, is a
    /// click target that opens that child. The first painted row also
    /// fills the single overlay status hit so older one-row tests still
    /// read `hit_overlay_nested_status`. Later named rows keep their
    /// own rect in `view_button_rects`.
    fn record_overlay_nested_status_hit(&mut self, id: String, hit: Rect) {
        self.tasks
            .view_button_rects
            .push((TaskEntryId::Agent(id.clone()), hit));
        if self.overlay_nested_status_child_sid.is_none() {
            self.hit_overlay_nested_status.set(Some(hit));
            self.overlay_nested_status_child_sid = Some(id);
        }
    }

    /// Left-click target for an overlay L3 open-arrow. The live L2 overlay
    /// tasks pane paints `[↗]` on the child view. Those hits open that L3
    /// on this view. Per-row overlay hits still win over the single status
    /// sid so two named specialists do not share the first painted sid.
    fn overlay_nested_open_sid_at(&self, column: u16, row: u16) -> Option<String> {
        let point = (column, row).into();
        // Live L2 overlay: the inner tasks pane `[↗]` is the control the
        // Operator clicks. It lives on the child view, not on L1 hits.
        if let Some(child_sid) = self.active_subagent.as_deref()
            && let Some(child) = self.subagent_views.get(child_sid)
        {
            for (id, rect) in &child.tasks.view_button_rects {
                if rect.contains(point)
                    && let TaskEntryId::Agent(sid) = id
                {
                    // Native chip stores TaskEntry.subagent_id (sa-l3-first),
                    // not the L1 view key (l3-first). Match registry key,
                    // subagent_id, or child_session_id, then pick the id that
                    // is actually in subagent_views. Skip the L2 floor.
                    if self.subagent_views.contains_key(sid) && sid.as_str() != child_sid {
                        return Some(sid.clone());
                    }
                    let native_sid = sid.as_str();
                    let resolved = self
                        .subagent_sessions
                        .iter()
                        .find(|(key, info)| {
                            key.as_str() == native_sid
                                || info.subagent_id.as_ref() == native_sid
                                || info.child_session_id.as_ref() == native_sid
                        })
                        .and_then(|(key, info)| {
                            [
                                info.child_session_id.as_ref(),
                                key.as_str(),
                                native_sid,
                                info.subagent_id.as_ref(),
                            ]
                            .into_iter()
                            .find(|id| self.subagent_views.contains_key(*id))
                            .map(str::to_owned)
                        });
                    if let Some(view_sid) = resolved
                        && view_sid != child_sid
                    {
                        return Some(view_sid);
                    }
                }
            }
        }
        for (id, rect) in &self.tasks.view_button_rects {
            if rect.contains(point)
                && let TaskEntryId::Agent(sid) = id
            {
                return Some(sid.clone());
            }
        }
        if self.hit_overlay_nested_status.contains(column, row) {
            self.overlay_nested_status_child_sid.clone()
        } else {
            None
        }
    }

    /// Parent-registry L3 rows are invisible to the L2 view's wait chrome.
    /// Mirror them for this draw so the overlay names the live specialist.
    fn mirror_parented_specialists_for_overlay(&mut self, child_sid: &str) -> Vec<String> {
        let already_on_child: std::collections::HashSet<String> = self
            .subagent_views
            .get(child_sid)
            .map(|child| child.subagent_sessions.keys().cloned().collect())
            .unwrap_or_default();
        let borrowed: Vec<(String, crate::app::subagent::SubagentInfo)> =
            crate::app::subagent::l3_rows_for_l2(self.subagent_sessions.values(), child_sid)
                .into_iter()
                .filter(|info| !already_on_child.contains(info.child_session_id.as_ref()))
                .map(|info| (info.child_session_id.to_string(), info.clone()))
                .collect();
        let Some(child) = self.subagent_views.get_mut(child_sid) else {
            return Vec::new();
        };
        let mut keys = Vec::new();
        for (id, info) in borrowed {
            child.subagent_sessions.insert(id.clone(), info);
            keys.push(id);
        }
        keys
    }

    fn drop_mirrored_overlay_specialists(&mut self, child_sid: &str, keys: &[String]) {
        let Some(child) = self.subagent_views.get_mut(child_sid) else {
            return;
        };
        for key in keys {
            child.subagent_sessions.remove(key);
        }
    }

    /// Test helper: ids `mirror_parented_specialists_for_overlay` would copy.
    /// Drops them again so the child map is unchanged. Does not change list behavior.
    #[cfg(test)]
    pub(crate) fn mirrored_overlay_specialist_ids_for_test(
        &mut self,
        child_sid: &str,
    ) -> Vec<String> {
        let keys = self.mirror_parented_specialists_for_overlay(child_sid);
        self.drop_mirrored_overlay_specialists(child_sid, &keys);
        keys
    }

    /// Idle child still listed as running: frame close cancels that row.
    fn completed_listed_overlay_kill_id(&self) -> Option<String> {
        let sid = self.active_subagent.as_deref()?;
        let child_idle = self
            .subagent_views
            .get(sid)
            .is_some_and(|view| !view.session.state.is_turn_running());
        if !child_idle {
            return None;
        }
        let info = self.subagent_sessions.get(sid)?;
        if !info.is_running() {
            return None;
        }
        Some(info.subagent_id.to_string())
    }
    /// `None` when no takeover is open, so the caller continues its normal routing. Otherwise all input goes to the
    /// child view; `q`/`Esc` from bare scrollback closes the view and Ctrl+Q always bubbles to the global quit.
    /// Rung order is fixed: Ctrl+Q, `[✗]` click, specialist status click, hover, bare-scrollback close, L2 floor, idle-quote, forward + filter.
    pub(super) fn intercept_takeover_input(
        &mut self,
        ev: &Event,
        registry: &ActionRegistry,
        prompt_paging: bool,
    ) -> Option<InputOutcome> {
        let child_sid = self.active_subagent.clone()?;
        if let Event::Paste(text) = ev
            && Self::bracketed_paste_waits_for_image_probe(text)
            && self.l2_overlay_composer_awaits_image_paste()
        {
            return None;
        }
        let key = match ev {
            Event::Key(key) if key.kind != KeyEventKind::Release => Some(key),
            _ => None,
        };
        if key.is_some_and(|key| key!('q', CONTROL).matches(key)) {
            return Some(InputOutcome::Unchanged);
        }
        if let Event::Mouse(mouse) = ev
            && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self
                .hit_subagent_frame_close
                .contains(mouse.column, mouse.row)
        {
            let kill_id = self.completed_listed_overlay_kill_id();
            self.close_subagent_fullscreen();
            if let Some(id) = kill_id {
                return Some(InputOutcome::Action(Action::KillSubagent(id)));
            }
            return Some(InputOutcome::Changed);
        }
        if let Event::Mouse(mouse) = ev
            && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && let Some(sid) = self.overlay_nested_open_sid_at(mouse.column, mouse.row)
            && self.subagent_views.contains_key(&sid)
        {
            let open_is_l2_floor = self
                .subagent_views
                .get(&child_sid)
                .is_some_and(|view| view.composer_route() == super::ComposerRoute::L2Floor);
            if open_is_l2_floor {
                self.overlay_return_sid = Some(child_sid.clone());
                self.active_subagent = Some(sid);
            } else {
                self.open_subagent_fullscreen(sid);
            }
            return Some(InputOutcome::Changed);
        }
        if let Event::Mouse(mouse) = ev
            && matches!(mouse.kind, MouseEventKind::Moved)
            && self
                .hit_subagent_frame_close
                .update_hover(mouse.column, mouse.row)
        {
            return Some(InputOutcome::Changed);
        }
        let child_in_scrollback = self
            .subagent_views
            .get(&child_sid)
            .is_some_and(|c| c.is_bare_scrollback());
        if child_in_scrollback
            && key.is_some_and(|key| key!('q').matches(key) || key.code == KeyCode::Esc)
        {
            self.close_subagent_fullscreen();
            return Some(InputOutcome::Changed);
        }
        let l2_floor = self
            .subagent_views
            .get(&child_sid)
            .is_some_and(|view| view.composer_route() == super::ComposerRoute::L2Floor);
        if l2_floor {
            if key
                .is_some_and(|key| key.code == KeyCode::Esc && key.modifiers == KeyModifiers::NONE)
            {
                self.close_subagent_fullscreen();
                return Some(InputOutcome::Changed);
            }
            if key.is_some_and(|key| key!('q').matches(key)) {
                let draft_empty = self
                    .subagent_views
                    .get(&child_sid)
                    .map(|view| view.prompt.text().trim().is_empty())
                    .unwrap_or(true);
                if draft_empty {
                    self.close_subagent_fullscreen();
                    return Some(InputOutcome::Changed);
                }
            }
            if key.is_some_and(|key| {
                key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE
            }) {
                let draft = self
                    .subagent_views
                    .get(&child_sid)
                    .map(|view| view.prompt.text().to_string())
                    .unwrap_or_default();
                if !draft.trim().is_empty() {
                    let _ = self.l2_floor_send_interject(&child_sid, draft);
                    return Some(InputOutcome::Changed);
                }
                if self.open_l3_from_l2() {
                    return Some(InputOutcome::Changed);
                }
            }
        }
        let child_quote = key.and_then(|key| {
            self.subagent_views
                .get_mut(&child_sid)
                .map(|child| child.try_take_idle_enter_quote(key))
        });
        match child_quote {
            Some(IdleEnterQuote::Quoted(quoted)) => {
                self.close_subagent_fullscreen();
                self.insert_quoted_reply(&quoted);
                return Some(InputOutcome::Changed);
            }
            Some(IdleEnterQuote::ConsumedEmpty) => return Some(InputOutcome::Changed),
            Some(IdleEnterQuote::NotHandled) | None => {}
        }
        let outcome = {
            let Some(child_view) = self.subagent_views.get_mut(&child_sid) else {
                return Some(InputOutcome::Unchanged);
            };
            let outcome = child_view.handle_input_inner(ev, registry, prompt_paging);
            let mut child_effects = std::mem::take(&mut child_view.pending_effects);
            self.pending_effects.append(&mut child_effects);
            outcome
        };
        if l2_floor {
            let redirected = match &outcome {
                InputOutcome::Action(Action::SendPrompt(text)) if !text.trim().is_empty() => {
                    Some(text.clone())
                }
                InputOutcome::Action(Action::Interject { text, .. }) if !text.trim().is_empty() => {
                    Some(text.clone())
                }
                _ => None,
            };
            if let Some(text) = redirected {
                let _ = self.l2_floor_send_interject(&child_sid, text);
                return Some(InputOutcome::Changed);
            }
        }
        Some(filter_child_outcome(outcome))
    }
}

const WAITING_ON_ONE_TASK: &str = "Waiting on 1 task...";
const WAITING_ON_TASK_OUTPUT: &str = "Waiting on task output…";
const ONE_SUBAGENT_STILL_RUNNING: &str = "1 subagent still running";

fn overlay_nested_specialist_count(view: &AgentView, child_sid: &str) -> usize {
    crate::app::subagent::l3_rows_for_l2(view.subagent_sessions.values(), child_sid).len()
}

/// One empty-description L3 wait keeps the title phrase and needs a
/// waiting row strictly below that title for `[↗]`. The reserved row is
/// the first inner content row (below the title border). None when the
/// title is not that phrase, the specialist is named, or inner is too
/// short to give the child a row after the steal.
fn reserved_one_task_wait_row(
    view: &AgentView,
    child_sid: &str,
    activity: Option<&str>,
    title_y: u16,
    inner: Rect,
) -> Option<Rect> {
    if activity != Some(WAITING_ON_ONE_TASK) {
        return None;
    }
    if inner.height <= 4 || inner.y <= title_y {
        return None;
    }
    let specialists =
        crate::app::subagent::l3_rows_for_l2(view.subagent_sessions.values(), child_sid);
    if specialists.len() != 1 {
        return None;
    }
    if !specialists
        .first()
        .is_some_and(|info| info.description.trim().is_empty())
    {
        return None;
    }
    Some(Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    })
}

/// Paint `Waiting on 1 task...` on `row` when that row does not already
/// carry the phrase. Used for the reserved waiting row below the title.
fn paint_waiting_phrase_on_row(buf: &mut Buffer, row: Rect) {
    if row.height == 0 || row.width == 0 {
        return;
    }
    if painted_phrase_rect_on_row(buf, row, row.y, WAITING_ON_ONE_TASK).is_some() {
        return;
    }
    let theme = Theme::current();
    buf.set_span_safe(
        row.x,
        row.y,
        &Span::styled(WAITING_ON_ONE_TASK, Style::default().fg(theme.gray)),
        row.width,
    );
}

/// A one-task wait's bottom status still says `1 subagent still running`.
/// Rewrite that lowest cue so the row paints `Waiting on 1 task...`.
/// The title row may keep the same words. Leave the row alone when it
/// already paints the waiting phrase.
fn paint_one_task_wait_over_still_running(buf: &mut Buffer, area: Rect) {
    paint_one_task_wait_over_phrase(buf, area, ONE_SUBAGENT_STILL_RUNNING);
}

/// Paint `[↗]` as an open control for an overlay phrase.
/// Named L3 rows and the one-task wait row get the arrow after the phrase.
/// Wait chrome that already paints `{description}: {tool}` keeps that last
/// tool; the arrow sits in the blank gap left of the description so the
/// two do not share glyphs (`[↗]ad_file` from `read_file`). A clamped
/// `{prefix}…: {tool}` row is the same last-tool chrome. When that left
/// gap is the spinner, the arrow sits after the tool or over the spinner
/// so a normal-width L2 overlay still shows the button. The hit covers
/// the phrase and that arrow.
fn paint_waiting_row_open_arrow(buf: &mut Buffer, area: Rect, phrase: Rect) -> Rect {
    let arrow = crate::glyphs::enlarge_button();
    let arrow_w: u16 = 3;
    let right = area.x.saturating_add(area.width);
    if phrase.height == 0 || area.width < arrow_w {
        return phrase;
    }
    let after_phrase = phrase.x.saturating_add(phrase.width);
    let arrow_x = if phrase_followed_by_last_tool_sep(buf, phrase) {
        match open_arrow_x_left_of_last_tool(buf, area, phrase, arrow_w)
            .or_else(|| open_arrow_x_after_last_tool(buf, area, phrase, arrow_w))
            .or_else(|| open_arrow_x_over_spinner(area, phrase, arrow_w))
        {
            Some(x) => x,
            None => return phrase,
        }
    } else if after_phrase.saturating_add(1).saturating_add(arrow_w) <= right {
        after_phrase.saturating_add(1)
    } else if after_phrase.saturating_add(arrow_w) <= right {
        after_phrase
    } else {
        match open_arrow_x_over_spinner(area, phrase, arrow_w) {
            Some(x) => x,
            None => return phrase,
        }
    };
    let theme = Theme::current();
    buf.set_span_safe(
        arrow_x,
        phrase.y,
        &Span::styled(arrow, Style::default().fg(theme.gray)),
        arrow_w,
    );
    let hit_x = arrow_x.min(phrase.x);
    let hit_end = after_phrase.max(arrow_x.saturating_add(arrow_w));
    Rect::new(hit_x, phrase.y, hit_end.saturating_sub(hit_x), 1)
}

/// `{description}: {tool}` wait chrome, including a clamped
/// `{prefix}…: {tool}` subject. The colon is the last-tool separator,
/// not room for `[↗]`.
fn phrase_followed_by_last_tool_sep(buf: &Buffer, phrase: Rect) -> bool {
    if phrase.height == 0 {
        return false;
    }
    let y = phrase.y;
    let mut x = phrase.x.saturating_add(phrase.width);
    x = skip_ellipsis_cells(buf, x, y);
    buf.cell((x, y)).is_some_and(|cell| cell.symbol() == ":")
}

fn cells_are_blank(buf: &Buffer, x: u16, y: u16, width: u16) -> bool {
    (0..width).all(|i| {
        buf.cell((x.saturating_add(i), y)).is_none_or(|cell| {
            let symbol = cell.symbol();
            symbol.is_empty() || symbol == " "
        })
    })
}

/// Sit `[↗]` in the blank gap left of `{description}: {tool}` so the last
/// tool stays whole. None when that gap is too short or already painted.
fn open_arrow_x_left_of_last_tool(
    buf: &Buffer,
    area: Rect,
    phrase: Rect,
    arrow_w: u16,
) -> Option<u16> {
    let gap: u16 = 1;
    let left_room = phrase.x.saturating_sub(area.x);
    let candidate = if left_room >= arrow_w.saturating_add(gap) {
        phrase.x.saturating_sub(arrow_w.saturating_add(gap))
    } else if left_room >= arrow_w {
        phrase.x.saturating_sub(arrow_w)
    } else {
        return None;
    };
    (candidate >= area.x && cells_are_blank(buf, candidate, phrase.y, arrow_w)).then_some(candidate)
}

fn is_ellipsis_symbol(symbol: &str) -> bool {
    symbol == "\u{2026}"
}

/// Skip a painted truncation ellipsis (`…` or `. . .`) so `{prefix}…: {tool}`
/// still reads as last-tool chrome.
fn skip_ellipsis_cells(buf: &Buffer, mut x: u16, y: u16) -> u16 {
    if buf
        .cell((x, y))
        .is_some_and(|cell| is_ellipsis_symbol(cell.symbol()))
    {
        return x.saturating_add(1);
    }
    let dots = (0..3).all(|i| {
        buf.cell((x.saturating_add(i), y))
            .is_some_and(|cell| cell.symbol() == ".")
    });
    if dots {
        x = x.saturating_add(3);
    }
    x
}

/// Sit `[↗]` after `{desc}: {tool}` (and a trailing ellipsis) when the left
/// gap is the spinner. None when that run would leave the row.
fn open_arrow_x_after_last_tool(
    buf: &Buffer,
    area: Rect,
    phrase: Rect,
    arrow_w: u16,
) -> Option<u16> {
    let y = phrase.y;
    let right = area.x.saturating_add(area.width);
    let mut x = skip_ellipsis_cells(buf, phrase.x.saturating_add(phrase.width), y);
    if !buf.cell((x, y)).is_some_and(|cell| cell.symbol() == ":") {
        return None;
    }
    x = x.saturating_add(1);
    if buf.cell((x, y)).is_some_and(|cell| cell.symbol() == " ") {
        x = x.saturating_add(1);
    }
    while x < right {
        let symbol = buf
            .cell((x, y))
            .map(|cell| cell.symbol().to_owned())
            .unwrap_or_default();
        if symbol.is_empty() || symbol == " " || is_ellipsis_symbol(&symbol) {
            break;
        }
        x = x.saturating_add(1);
    }
    x = skip_ellipsis_cells(buf, x, y);
    let with_gap = x.saturating_add(1);
    if with_gap.saturating_add(arrow_w) <= right && cells_are_blank(buf, with_gap, y, arrow_w) {
        return Some(with_gap);
    }
    if x.saturating_add(arrow_w) <= right && cells_are_blank(buf, x, y, arrow_w) {
        return Some(x);
    }
    None
}

/// Last resort on a packed wait row: paint `[↗]` over the spinner left of
/// the phrase so a normal-width overlay still shows the button. Does not
/// sit on the phrase (the last-tool name stays whole).
fn open_arrow_x_over_spinner(area: Rect, phrase: Rect, arrow_w: u16) -> Option<u16> {
    let candidate = phrase.x.saturating_sub(arrow_w);
    (candidate >= area.x && candidate.saturating_add(arrow_w) <= phrase.x).then_some(candidate)
}

/// Painted specialist phrase, including a clamped wait-chrome prefix when
/// the full description is not on screen. Prefers a dedicated L3-row run,
/// then last-tool chrome.
fn painted_specialist_phrase_rect(buf: &Buffer, area: Rect, phrase: &str) -> Option<Rect> {
    painted_specialist_phrase_matching(buf, area, phrase, false)
}

/// Last-tool wait chrome (`{prefix}…: {tool}`), including a clamped prefix
/// when the full description is not on screen. Dedicated specialist-list
/// rows are skipped so wait chrome still hosts `[↗]` when both exist.
fn painted_last_tool_wait_chrome_phrase_rect(
    buf: &Buffer,
    area: Rect,
    phrase: &str,
) -> Option<Rect> {
    painted_specialist_phrase_matching(buf, area, phrase, true)
}

fn painted_specialist_phrase_matching(
    buf: &Buffer,
    area: Rect,
    phrase: &str,
    last_tool_only: bool,
) -> Option<Rect> {
    let phrase = phrase.trim();
    if phrase.is_empty() {
        return None;
    }
    if let Some(rect) = painted_open_control_phrase_rect_kind(buf, area, phrase, last_tool_only) {
        return Some(rect);
    }
    let clamped = crate::acp::tracker::clamp_activity_subject(phrase);
    if clamped != phrase
        && let Some(rect) =
            painted_open_control_phrase_rect_kind(buf, area, &clamped, last_tool_only)
    {
        return Some(rect);
    }
    let chars: Vec<char> = phrase.chars().collect();
    let min = 8.min(chars.len());
    if min == 0 {
        return None;
    }
    for len in (min..chars.len()).rev() {
        let Some(prefix_chars) = chars.get(..len) else {
            continue;
        };
        let prefix: String = prefix_chars.iter().copied().collect();
        if let Some(rect) =
            painted_open_control_phrase_rect_kind(buf, area, &prefix, last_tool_only)
        {
            return Some(rect);
        }
    }
    None
}

/// First dedicated L3-row paint of `phrase` (not `{desc}: {tool}` wait
/// chrome) unless `last_tool_only`. Last-tool-only skips specialist-list
/// rows so wait chrome still hosts `[↗]`. Otherwise falls back to the
/// first last-tool occurrence so that row can still host an open arrow
/// left of the tool name.
fn painted_open_control_phrase_rect_kind(
    buf: &Buffer,
    area: Rect,
    phrase: &str,
    last_tool_only: bool,
) -> Option<Rect> {
    let glyphs: Vec<String> = phrase.chars().map(|ch| ch.to_string()).collect();
    if glyphs.is_empty() || area.width == 0 || area.height == 0 {
        return None;
    }
    let bottom = area.y.saturating_add(area.height);
    let right = area.x.saturating_add(area.width);
    let mut last_tool_fallback = None;
    for y in area.y..bottom {
        let symbols: Vec<String> = (area.x..right)
            .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_owned()))
            .collect();
        if symbols.len() < glyphs.len() {
            continue;
        }
        for start in 0..=symbols.len() - glyphs.len() {
            let end = start + glyphs.len();
            if symbols.get(start..end) != Some(glyphs.as_slice()) {
                continue;
            }
            let width = u16::try_from(glyphs.len()).unwrap_or(u16::MAX);
            let x = area
                .x
                .saturating_add(u16::try_from(start).unwrap_or(u16::MAX));
            let rect = Rect::new(x, y, width, 1);
            if phrase_followed_by_last_tool_sep(buf, rect) {
                if last_tool_only {
                    return Some(rect);
                }
                if last_tool_fallback.is_none() {
                    last_tool_fallback = Some(rect);
                }
                break;
            }
            if last_tool_only {
                break;
            }
            return Some(rect);
        }
    }
    if last_tool_only {
        None
    } else {
        last_tool_fallback
    }
}

/// Rewrite the lowest run of `old` to `Waiting on 1 task...`, padded to the
/// old width so a longer unlabeled wait does not leave leftover glyphs.
fn paint_one_task_wait_over_phrase(buf: &mut Buffer, area: Rect, old: &str) {
    let Some(still) = lowest_painted_phrase_rect(buf, area, old) else {
        return;
    };
    if painted_phrase_rect_on_row(buf, area, still.y, WAITING_ON_ONE_TASK).is_some() {
        return;
    }
    let style = buf
        .cell((still.x, still.y))
        .map(|cell| cell.style())
        .unwrap_or_default();
    let phrase_width = u16::try_from(WAITING_ON_ONE_TASK.chars().count()).unwrap_or(still.width);
    let pad = still.width.saturating_sub(phrase_width);
    let text = format!("{WAITING_ON_ONE_TASK}{}", " ".repeat(pad as usize));
    buf.set_span_safe(
        still.x,
        still.y,
        &Span::styled(text.as_str(), style),
        still.width,
    );
}

/// Lowest one-row run of `phrase` inside `area`.
fn lowest_painted_phrase_rect(buf: &Buffer, area: Rect, phrase: &str) -> Option<Rect> {
    phrase_rect_scanning(buf, area, phrase, true)
}

/// First one-row run of `phrase` inside `area`. One cell per glyph, so the
/// rect's origin is the cell a left click on that status hits.
fn painted_phrase_rect(buf: &Buffer, area: Rect, phrase: &str) -> Option<Rect> {
    phrase_rect_scanning(buf, area, phrase, false)
}

fn painted_phrase_rect_on_row(buf: &Buffer, area: Rect, y: u16, phrase: &str) -> Option<Rect> {
    let row = Rect {
        y,
        height: 1,
        ..area
    };
    painted_phrase_rect(buf, row, phrase)
}

fn phrase_rect_scanning(buf: &Buffer, area: Rect, phrase: &str, lowest: bool) -> Option<Rect> {
    let glyphs: Vec<String> = phrase.chars().map(|ch| ch.to_string()).collect();
    if glyphs.is_empty() || area.width == 0 || area.height == 0 {
        return None;
    }
    let bottom = area.y.saturating_add(area.height);
    let right = area.x.saturating_add(area.width);
    let mut found = None;
    for y in area.y..bottom {
        let symbols: Vec<String> = (area.x..right)
            .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_owned()))
            .collect();
        if symbols.len() < glyphs.len() {
            continue;
        }
        for start in 0..=symbols.len() - glyphs.len() {
            let end = start + glyphs.len();
            if symbols.get(start..end) == Some(glyphs.as_slice()) {
                let width = u16::try_from(glyphs.len()).unwrap_or(u16::MAX);
                let x = area
                    .x
                    .saturating_add(u16::try_from(start).unwrap_or(u16::MAX));
                let rect = Rect::new(x, y, width, 1);
                if !lowest {
                    return Some(rect);
                }
                found = Some(rect);
                break;
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "subagent_takeover_tests.rs"]
mod tests;
