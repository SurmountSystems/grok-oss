<<<<<<< HEAD
=======
//! Settings modal keyboard and mouse input handling.

>>>>>>> e3fdf3ed (Merge 2 (#4))
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::layout::Rect;

use super::render::int_step_sizes;
use super::state::{
<<<<<<< HEAD
    RowEntry, SettingsKeyOutcome, SettingsModalState, SettingsMode, SettingsModeKind,
    action_for_bool, action_for_enum, action_for_enum_commit, action_for_int, action_for_string,
    effective_enum_choices, group_children, validate_string,
};
use crate::app::actions::Action;
use crate::input::line_editor::LineEditOutcome;
use crate::settings::{
    SettingKey, SettingKind, SettingValue, StringValidator, dynamic_enum_choices,
};

/// F2/Ctrl+,/Cmd+, always close regardless of mode. Space/Enter Repeat events are suppressed to
/// avoid per-tick disk writes.
=======
    RowEntry, SettingsKeyOutcome, SettingsModalMode, SettingsModalState, action_for_bool,
    action_for_enum, action_for_enum_commit, action_for_int, action_for_string,
    effective_enum_choices, group_children, validate_int, validate_string,
};
use crate::app::actions::Action;
use crate::settings::{SettingKey, SettingKind, SettingValue, dynamic_enum_choices};

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

/// Handle a key event in the settings modal.
///
/// F2/Ctrl+,/Cmd+, always close regardless of mode. Esc behavior is
/// mode-dependent: Browse delegates to chrome, sub-modes handle it
/// locally (FilterFocused clears query, PickingEnum reverts preview,
/// EditingValue cancels). Space/Enter Repeat events are suppressed
/// to avoid per-tick disk writes.
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub fn handle_settings_key(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    if key.kind == KeyEventKind::Release {
        return SettingsKeyOutcome::Unchanged;
    }

<<<<<<< HEAD
=======
    // Suppress Repeat for toggle keys to avoid per-tick disk writes.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    if key.kind == KeyEventKind::Repeat && matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
        return SettingsKeyOutcome::Unchanged;
    }

    if is_close_key(key) {
        return SettingsKeyOutcome::Close;
    }

<<<<<<< HEAD
    match state.state.mode_kind() {
        SettingsModeKind::Browse => handle_browse(state, key),
        SettingsModeKind::FilterFocused => handle_filter_focused(state, key),
        SettingsModeKind::PickingEnum => handle_picking_enum(state, key),
        SettingsModeKind::PickingGroup => handle_picking_group(state, key),
        SettingsModeKind::EditingString | SettingsModeKind::EditingInt => {
            handle_editing_value(state, key)
        }
    }
}

pub fn handle_settings_paste(state: &mut SettingsModalState, text: &str) -> SettingsKeyOutcome {
    match state.state.mode_kind() {
        SettingsModeKind::FilterFocused => {
            let outcome = state.state.filter.insert_paste(text);
            apply_filter_edit(state, outcome)
        }
        SettingsModeKind::EditingString => {
            let (validator, outcome) = {
                let SettingsMode::EditingString {
                    editor, validator, ..
                } = &mut state.state.mode
                else {
                    unreachable!("mode kind changed before paste")
                };
                (
                    *validator,
                    editor.insert_paste_with_policy(text, safe_settings_char, usize::MAX),
                )
            };
            apply_string_edit(state, validator, outcome)
        }
        SettingsModeKind::Browse
        | SettingsModeKind::PickingEnum
        | SettingsModeKind::PickingGroup
        | SettingsModeKind::EditingInt => SettingsKeyOutcome::Unchanged,
    }
}

/// Key routing for the enum chooser: Up/Down dispatches preview actions, Enter commits the current choice, Esc reverts to the original value.
fn handle_picking_enum(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    let (setting_key, choices_idx, original_value, supports_preview) = match &state.state.mode {
        SettingsMode::PickingEnum {
=======
    // Exhaustive per-mode dispatch.
    match state.mode {
        SettingsModalMode::Browse => handle_browse(state, key),
        SettingsModalMode::FilterFocused => handle_filter_focused(state, key),
        SettingsModalMode::PickingEnum { .. } => handle_picking_enum(state, key),
        SettingsModalMode::PickingGroup { .. } => handle_picking_group(state, key),
        SettingsModalMode::EditingValue { .. } => handle_editing_value(state, key),
    }
}

/// Enum chooser key routing. Up/Down dispatches preview actions,
/// Enter commits current choice, Esc reverts to original value.
fn handle_picking_enum(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    // Snapshot the current picker state under an immutable borrow so
    // the subsequent `state.mode = ...` writes are unambiguous.
    let (setting_key, choices_idx, original_value, supports_preview) = match &state.mode {
        SettingsModalMode::PickingEnum {
>>>>>>> e3fdf3ed (Merge 2 (#4))
            key,
            choices_idx,
            original_value,
            supports_preview,
        } => (
            *key,
            *choices_idx,
            original_value.clone(),
            *supports_preview,
        ),
<<<<<<< HEAD
        _ => unreachable!("picker handler requires PickingEnum state"),
=======
        _ => return SettingsKeyOutcome::Unchanged,
>>>>>>> e3fdf3ed (Merge 2 (#4))
    };

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            let len = picker_choices_len(state, setting_key);
            if choices_idx + 1 >= len {
                return SettingsKeyOutcome::Unchanged;
            }
            set_picker_idx(
                state,
                setting_key,
                choices_idx + 1,
                original_value,
                supports_preview,
            )
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if choices_idx == 0 {
                return SettingsKeyOutcome::Unchanged;
            }
            set_picker_idx(
                state,
                setting_key,
                choices_idx - 1,
                original_value,
                supports_preview,
            )
        }
        KeyCode::Enter => {
<<<<<<< HEAD
            // Commit the focused choice; this is the only point in a picker's open-to-close cycle that fires
            // `Effect::PersistSetting`.
            let close = std::mem::take(&mut state.close_on_picker_exit);
            if !close {
                state.transition_to_browse();
            }
=======
            // Commit: dispatch the typed COMMIT Action for the
            // currently-focused choice. This is the single place
            // per picker open → close cycle that fires
            // `Effect::PersistSetting`, eliminating the per-keystroke
            // disk write race. The most recent PREVIEW Action (from Up/Down) has
            // already mutated the live visual; the commit's
            // `set_X_inner` is idempotent on that.
            //
            // For
            // `SettingKind::DynamicEnum` settings (e.g.
            // `default_model`, `fork_secondary_model`), the picker
            // commits through `action_for_string` rather than
            // `action_for_enum_commit` — the canonical is a runtime
            // string sourced from the model catalog, which
            // `action_for_string` already knows how to resolve via
            // `snapshot.resolve_model_name` AND treats the empty
            // canonical as a `Clear*` sentinel.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            let kind_is_dynamic = matches!(
                state.registry.find(setting_key).map(|m| &m.kind),
                Some(SettingKind::DynamicEnum { .. })
            );
<<<<<<< HEAD
            let commit = if kind_is_dynamic {
                picker_choice_at_owned(state, setting_key, choices_idx).and_then(|canonical| {
                    action_for_string(setting_key, canonical, &state.pager_snapshot)
                })
            } else {
                picker_choice_at(state, setting_key, choices_idx)
                    .and_then(|c| action_for_enum_commit(setting_key, c))
            };
            match (close, commit) {
                (true, Some(action)) => SettingsKeyOutcome::ActionThenClose(action),
                (true, None) => SettingsKeyOutcome::Close,
                (false, Some(action)) => SettingsKeyOutcome::Action(action),
                (false, None) => SettingsKeyOutcome::Changed,
            }
        }
        KeyCode::Esc => {
            let close = std::mem::take(&mut state.close_on_picker_exit);
            if !close {
                state.transition_to_browse();
            }
            if let SettingValue::Enum(orig) = &original_value
                && let Some(action) = action_for_enum(setting_key, orig)
            {
                return if close {
                    SettingsKeyOutcome::ActionThenClose(action)
                } else {
                    SettingsKeyOutcome::Action(action)
                };
            }
            if close {
                SettingsKeyOutcome::Close
            } else {
                SettingsKeyOutcome::Changed
            }
        }
        // `d` reset: close the picker, revert the preview if applicable, then open the reset-confirm overlay
        // Consent choosers opt out entirely (no footer hint, no hidden shortcut); reset stays reachable from the browse row
        KeyCode::Char('d')
            if key.modifiers.is_empty() && !crate::settings::is_consent_chooser(setting_key) =>
        {
=======
            state.transition_to_browse();
            if kind_is_dynamic {
                let Some(canonical) = picker_choice_at_owned(state, setting_key, choices_idx)
                else {
                    return SettingsKeyOutcome::Changed;
                };
                if let Some(action) =
                    action_for_string(setting_key, canonical, &state.pager_snapshot)
                {
                    return SettingsKeyOutcome::Action(action);
                }
                return SettingsKeyOutcome::Changed;
            }
            let Some(current_canonical) = picker_choice_at(state, setting_key, choices_idx) else {
                return SettingsKeyOutcome::Changed;
            };
            if let Some(action) = action_for_enum_commit(setting_key, current_canonical) {
                return SettingsKeyOutcome::Action(action);
            }
            SettingsKeyOutcome::Changed
        }
        KeyCode::Esc => {
            // Revert preview and return to Browse. Non-preview Enums
            // skip the revert (no live visual was applied).
            state.transition_to_browse();
            if let SettingValue::Enum(orig) = &original_value
                && let Some(action) = action_for_enum(setting_key, orig)
            {
                return SettingsKeyOutcome::Action(action);
            }
            SettingsKeyOutcome::Changed
        }
        // `d` reset: close picker, revert preview if applicable,
        // then open the reset-confirm overlay.
        KeyCode::Char('d') if key.modifiers.is_empty() => {
>>>>>>> e3fdf3ed (Merge 2 (#4))
            state.transition_to_browse();
            if supports_preview
                && let SettingValue::Enum(orig) = &original_value
                && let Some(revert) = action_for_enum(setting_key, orig)
            {
                return SettingsKeyOutcome::ActionPair(
                    revert,
                    Action::OpenResetConfirm { key: setting_key },
                );
            }
            SettingsKeyOutcome::Action(Action::OpenResetConfirm { key: setting_key })
        }
        _ => SettingsKeyOutcome::Unchanged,
    }
}

<<<<<<< HEAD
/// Key routing for the group sub-sheet.
/// Up/Down moves between the child toggles; Space/Enter toggles the focused child in place (the sheet stays open); Esc returns to Browse.
fn handle_picking_group(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    let (group_key, child_idx) = match &state.state.mode {
        SettingsMode::PickingGroup { key, child_idx } => (*key, *child_idx),
        _ => unreachable!("group handler requires PickingGroup state"),
    };
    let children = group_children(state, group_key);
    if children.is_empty() {
        // Defensive: a group with no children can't be navigated, so back out
=======
/// Group sub-sheet key routing. Up/Down moves between the child toggles;
/// Space/Enter toggles the focused child in place (the sheet stays open);
/// Esc returns to Browse.
fn handle_picking_group(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    let (group_key, child_idx) = match &state.mode {
        SettingsModalMode::PickingGroup { key, child_idx } => (*key, *child_idx),
        _ => return SettingsKeyOutcome::Unchanged,
    };
    let children = group_children(state, group_key);
    if children.is_empty() {
        // Defensive: a group with no children can't be navigated — back out.
>>>>>>> e3fdf3ed (Merge 2 (#4))
        state.transition_to_browse();
        return SettingsKeyOutcome::Changed;
    }

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if child_idx + 1 >= children.len() {
                return SettingsKeyOutcome::Unchanged;
            }
<<<<<<< HEAD
            state.transition_to_picking_group(group_key, child_idx + 1);
=======
            state.mode = SettingsModalMode::PickingGroup {
                key: group_key,
                child_idx: child_idx + 1,
            };
>>>>>>> e3fdf3ed (Merge 2 (#4))
            SettingsKeyOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if child_idx == 0 {
                return SettingsKeyOutcome::Unchanged;
            }
<<<<<<< HEAD
            state.transition_to_picking_group(group_key, child_idx - 1);
            SettingsKeyOutcome::Changed
        }
        // Space/Enter toggle the focused child Bool and stay in the sheet so the user can flip several tips in a row
        // The dispatcher refreshes the modal snapshot, so the new value paints on the next frame
=======
            state.mode = SettingsModalMode::PickingGroup {
                key: group_key,
                child_idx: child_idx - 1,
            };
            SettingsKeyOutcome::Changed
        }
        // Space/Enter toggle the focused child Bool and stay in the sheet so the
        // user can flip several tips in a row. The dispatcher refreshes the
        // modal snapshot, so the new value paints on the next frame.
>>>>>>> e3fdf3ed (Merge 2 (#4))
        KeyCode::Char(' ') | KeyCode::Enter => {
            let Some(child_key) = children.get(child_idx).copied() else {
                return SettingsKeyOutcome::Unchanged;
            };
            let cur = match state.value_for(child_key) {
                Some(SettingValue::Bool(b)) => b,
                _ => return SettingsKeyOutcome::Unchanged,
            };
            match action_for_bool(child_key, !cur) {
                Some(action) => SettingsKeyOutcome::Action(action),
                None => SettingsKeyOutcome::Unchanged,
            }
        }
        KeyCode::Esc => {
            state.transition_to_browse();
            SettingsKeyOutcome::Changed
        }
        _ => SettingsKeyOutcome::Unchanged,
    }
}

<<<<<<< HEAD
/// Shared nav body for the picker's Up/Down (and j/k aliases).
/// Updates `choices_idx` in place, looks up the new canonical, and fires the preview dispatch via `action_for_enum`.
/// `new_idx` must be in bounds. Preview only fires for Enums with `supports_preview: true`; side-effecting Enums skip it.
=======
/// Common nav body for Up/Down (and j/k aliases) in the picker:
/// update `choices_idx` in-place, look up the new canonical, fire
/// the preview dispatch via `action_for_enum`. Extracted from the
/// Update picker index and optionally dispatch a preview action.
/// `new_idx` must be in-bounds. Preview only fires for Enums with
/// `supports_preview: true`; side-effecting Enums skip preview.
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub(super) fn set_picker_idx(
    state: &mut SettingsModalState,
    setting_key: SettingKey,
    new_idx: usize,
    original_value: SettingValue,
    supports_preview: bool,
) -> SettingsKeyOutcome {
    let in_bounds = new_idx < picker_choices_len(state, setting_key);
    if !in_bounds {
<<<<<<< HEAD
        // The caller bounds-checks already; this re-check protects a future caller that does not
        return SettingsKeyOutcome::Unchanged;
    }
    // Focus moves cancel a pending double-click.
    state.picker_last_click = None;
    state.transition_to_picking_enum(setting_key, new_idx, original_value, supports_preview);
=======
        // Caller bounds-checks before calling; belt-and-suspenders
        // for refactor safety.
        return SettingsKeyOutcome::Unchanged;
    }
    state.mode = SettingsModalMode::PickingEnum {
        key: setting_key,
        choices_idx: new_idx,
        supports_preview,
        original_value,
    };
>>>>>>> e3fdf3ed (Merge 2 (#4))
    // Preview dispatch for static Enums with preview support.
    if supports_preview
        && let Some(new_canonical) = picker_choice_at(state, setting_key, new_idx)
        && let Some(action) = action_for_enum(setting_key, new_canonical)
    {
        return SettingsKeyOutcome::Action(action);
    }
    SettingsKeyOutcome::Changed
}

<<<<<<< HEAD
/// Key routing for the inline string/int editor. Esc cancels, Enter commits.
/// String mode is free-form text with a cursor.
/// Int mode is a range-aware stepper (Up/Down small, Left/Right large; see [`int_step_sizes`]), clamped to [min,max].
fn handle_editing_value(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    // Int settings dispatch through a stepper-only handler
    // Char input, cursor panning, Backspace, Delete, Home, and End are rejected; only Up/Down/Left/Right (and j/k/h/l), Enter, and Esc do anything
    if let SettingsMode::EditingInt {
        key: setting_key,
        buffer,
        min,
        max,
    } = &state.state.mode
    {
        let setting_key = *setting_key;
        let buffer = buffer.clone();
        return handle_int_stepper(state, key, setting_key, &buffer, *min, *max);
    }

    let (setting_key, validator) = match &state.state.mode {
        SettingsMode::EditingString { key, validator, .. } => (*key, *validator),
        _ => unreachable!("editing handler requires String or Int state"),
    };

    if key.code == KeyCode::Enter {
        let SettingsMode::EditingString { editor, .. } = &state.state.mode else {
            unreachable!("String editor state changed during commit");
        };
        let text = editor.text().to_owned();
        let error = validate_string(validator, &text, &state.pager_snapshot.available_models);
        if error.is_some() {
            let SettingsMode::EditingString {
                validation_error, ..
            } = &mut state.state.mode
            else {
                unreachable!("String editor state changed during validation");
            };
            *validation_error = error;
            return SettingsKeyOutcome::Unchanged;
        }
        let action = action_for_string(setting_key, text, &state.pager_snapshot);
        state.transition_to_browse();
        return match action {
            Some(action) => SettingsKeyOutcome::Action(action),
            None => {
                tracing::error!(
                    target: "settings",
                    key = setting_key,
                    "EditingValue commit has no action_for_string arm — registry skew",
                );
                SettingsKeyOutcome::Changed
            }
        };
    }

    if key.code == KeyCode::Esc {
        state.transition_to_browse();
        return SettingsKeyOutcome::Changed;
    }

    if matches!(
        key.code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Tab
            | KeyCode::BackTab
    ) {
        return SettingsKeyOutcome::Unchanged;
    }

    let outcome = {
        let SettingsMode::EditingString { editor, .. } = &mut state.state.mode else {
            unreachable!("String editor state changed before key handling");
        };
        editor.handle_key_with_insert_policy(key, safe_settings_char)
    };
    apply_string_edit(state, validator, outcome)
}

fn apply_string_edit(
    state: &mut SettingsModalState,
    validator: StringValidator,
    outcome: LineEditOutcome,
) -> SettingsKeyOutcome {
    match outcome {
        LineEditOutcome::TextChanged => {
            let SettingsMode::EditingString { editor, .. } = &state.state.mode else {
                unreachable!("String editor state changed after text mutation");
            };
            let error = validate_string(
                validator,
                editor.text(),
                &state.pager_snapshot.available_models,
            );
            let SettingsMode::EditingString {
                validation_error, ..
            } = &mut state.state.mode
            else {
                unreachable!("String editor state changed during validation");
            };
            *validation_error = error;
            SettingsKeyOutcome::Changed
        }
        LineEditOutcome::HandledNoChange | LineEditOutcome::CursorChanged => {
            SettingsKeyOutcome::Changed
        }
        LineEditOutcome::Unhandled => SettingsKeyOutcome::Unchanged,
    }
}

/// Steps by range-aware small (Up/Down) or large (Left/Right) deltas from [`int_step_sizes`], clamped to [min,max].
=======
/// Inline string/int editor key routing. Esc cancels, Enter commits.
/// String mode: free-form text with cursor. Int mode: range-aware stepper
/// (Up/Down small, Left/Right large; see [`int_step_sizes`]), clamped to [min,max].
fn handle_editing_value(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    // Snapshot mode payload under an immutable borrow.
    let (setting_key, buffer, cursor_byte, validation_error) = match &state.mode {
        SettingsModalMode::EditingValue {
            key,
            buffer,
            cursor_byte,
            validation_error,
        } => (*key, buffer.clone(), *cursor_byte, validation_error.clone()),
        _ => return SettingsKeyOutcome::Unchanged,
    };

    // Look up the registered kind so we know how to handle this
    // edit. The lookup is `&self`-only.
    let Some(meta) = state.registry.find(setting_key) else {
        // Registry skew — log and exit. The CI guards catch this.
        tracing::error!(
            target: "settings",
            key = setting_key,
            "EditingValue mode references an unregistered key — exiting to Browse",
        );
        state.transition_to_browse();
        return SettingsKeyOutcome::Changed;
    };
    let kind_snapshot = meta.kind.clone();

    // Int settings dispatch through a stepper-only
    // handler. All char-input / cursor-pan / Backspace / Delete /
    // Home / End keys are rejected; only Up/Down/Left/Right (and
    // j/k/h/l aliases), Enter, and Esc do anything.
    if matches!(kind_snapshot, SettingKind::Int { .. }) {
        return handle_int_stepper(state, key, setting_key, &buffer, &kind_snapshot);
    }

    match key.code {
        KeyCode::Esc => {
            state.transition_to_browse();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Enter => {
            // Commit gate: re-validate against the current buffer.
            // On failure, refresh the inline error and stay in
            // EditingValue.
            let error = match &kind_snapshot {
                SettingKind::String { validator, .. } => {
                    validate_string(*validator, &buffer, &state.pager_snapshot.available_models)
                }
                _ => return SettingsKeyOutcome::Unchanged,
            };
            if error.is_some() {
                update_editing_value_buffer(state, buffer, cursor_byte, error);
                return SettingsKeyOutcome::Unchanged;
            }
            // Dispatch the typed Action and transition to Browse.
            let action_opt = match &kind_snapshot {
                SettingKind::String { .. } => {
                    action_for_string(setting_key, buffer.clone(), &state.pager_snapshot)
                }
                _ => None,
            };
            state.transition_to_browse();
            match action_opt {
                Some(action) => SettingsKeyOutcome::Action(action),
                None => {
                    tracing::error!(
                        target: "settings",
                        key = setting_key,
                        "EditingValue commit has no action_for_string arm — registry skew",
                    );
                    SettingsKeyOutcome::Changed
                }
            }
        }
        KeyCode::Backspace => {
            if cursor_byte == 0 {
                return SettingsKeyOutcome::Unchanged;
            }
            let mut new_buf = buffer.clone();
            // Find the prev char boundary.
            let prev = (0..cursor_byte)
                .rev()
                .find(|&i| new_buf.is_char_boundary(i))
                .unwrap_or(0);
            new_buf.replace_range(prev..cursor_byte, "");
            let new_cursor = prev;
            let new_error = recompute_validation(&kind_snapshot, &new_buf, state);
            update_editing_value_buffer(state, new_buf, new_cursor, new_error);
            SettingsKeyOutcome::Changed
        }
        KeyCode::Delete => {
            if cursor_byte >= buffer.len() {
                return SettingsKeyOutcome::Unchanged;
            }
            let mut new_buf = buffer.clone();
            // Find next char boundary.
            let next = (cursor_byte + 1..=new_buf.len())
                .find(|&i| new_buf.is_char_boundary(i))
                .unwrap_or(new_buf.len());
            new_buf.replace_range(cursor_byte..next, "");
            let new_error = recompute_validation(&kind_snapshot, &new_buf, state);
            update_editing_value_buffer(state, new_buf, cursor_byte, new_error);
            SettingsKeyOutcome::Changed
        }
        KeyCode::Left => {
            if cursor_byte == 0 {
                return SettingsKeyOutcome::Unchanged;
            }
            let prev = (0..cursor_byte)
                .rev()
                .find(|&i| buffer.is_char_boundary(i))
                .unwrap_or(0);
            update_editing_value_buffer(state, buffer, prev, validation_error);
            SettingsKeyOutcome::Changed
        }
        KeyCode::Right => {
            if cursor_byte >= buffer.len() {
                return SettingsKeyOutcome::Unchanged;
            }
            let next = (cursor_byte + 1..=buffer.len())
                .find(|&i| buffer.is_char_boundary(i))
                .unwrap_or(buffer.len());
            update_editing_value_buffer(state, buffer, next, validation_error);
            SettingsKeyOutcome::Changed
        }
        KeyCode::Home => {
            update_editing_value_buffer(state, buffer, 0, validation_error);
            SettingsKeyOutcome::Changed
        }
        KeyCode::End => {
            let end = buffer.len();
            update_editing_value_buffer(state, buffer, end, validation_error);
            SettingsKeyOutcome::Changed
        }
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            // Defense-in-depth for the (currently unused) String editor path:
            // reject control + bidi/format chars so a future `String` setting
            // can't reintroduce the Trojan-Source surface.
            let accept = match &kind_snapshot {
                SettingKind::String { .. } => !crate::render::line_utils::is_unsafe_display_char(c),
                _ => false,
            };
            if !accept {
                return SettingsKeyOutcome::Unchanged;
            }
            let mut new_buf = buffer.clone();
            new_buf.insert(cursor_byte, c);
            let new_cursor = cursor_byte + c.len_utf8();
            let new_error = recompute_validation(&kind_snapshot, &new_buf, state);
            update_editing_value_buffer(state, new_buf, new_cursor, new_error);
            SettingsKeyOutcome::Changed
        }
        _ => SettingsKeyOutcome::Unchanged,
    }
}

/// Int stepper handler. Steps by range-aware small (Up/Down) or large
/// (Left/Right) deltas from [`int_step_sizes`], clamped to [min,max].
>>>>>>> e3fdf3ed (Merge 2 (#4))
/// Non-stepper keys are rejected.
fn handle_int_stepper(
    state: &mut SettingsModalState,
    key: &KeyEvent,
    setting_key: SettingKey,
    buffer: &str,
<<<<<<< HEAD
    min: i64,
    max: i64,
) -> SettingsKeyOutcome {
    let (small_step, large_step) = int_step_sizes(min, max);
=======
    kind: &SettingKind,
) -> SettingsKeyOutcome {
    let SettingKind::Int { min, max, .. } = kind else {
        // Caller pre-checked the kind; defensive bail.
        return SettingsKeyOutcome::Unchanged;
    };

    let (small_step, large_step) = int_step_sizes(*min, *max);
>>>>>>> e3fdf3ed (Merge 2 (#4))
    let step_delta = |dir: i64, large: bool| -> i64 {
        let magnitude = if large { large_step } else { small_step };
        dir * magnitude
    };

    let apply_step = |state: &mut SettingsModalState, delta: i64| -> SettingsKeyOutcome {
<<<<<<< HEAD
        let cur = buffer.parse::<i64>().unwrap_or(min);
        let new = cur.saturating_add(delta).clamp(min, max);
        if new == cur {
            // Already clamped, so no visible change
            // Report Unchanged so the `clamps_to_min/max` tests can distinguish a no-op from a step
            return SettingsKeyOutcome::Unchanged;
        }
        let new_buf = new.to_string();
        update_int_buffer(state, new_buf);
        SettingsKeyOutcome::Changed
    };

    // Only modifier-free or SHIFT+arrow events trigger the stepper
    // Ctrl/Alt/etc belong to other editor features (selection extend, history) that the stepper has no notion of
=======
        let cur = buffer.parse::<i64>().unwrap_or(*min);
        let new = cur.saturating_add(delta).clamp(*min, *max);
        if new == cur {
            // Already clamped — no visible change. Report
            // Unchanged so the test for `clamps_to_min/max` can
            // distinguish a no-op from a step.
            return SettingsKeyOutcome::Unchanged;
        }
        let new_buf = new.to_string();
        let new_cursor = new_buf.len();
        update_editing_value_buffer(state, new_buf, new_cursor, None);
        SettingsKeyOutcome::Changed
    };

    // Only modifier-free or SHIFT+arrow events should trigger the
    // stepper; Ctrl/Alt/etc carry editor-unrelated semantics
    // (selection extend, history, etc) that the stepper has no
    // notion of.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    if !(key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) {
        return SettingsKeyOutcome::Unchanged;
    }

    match key.code {
        KeyCode::Esc => {
            state.transition_to_browse();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Enter => {
<<<<<<< HEAD
            // Commit. The buffer is guaranteed in range by the clamp on every step; parse it and dispatch.
=======
            // Commit. Buffer is guaranteed in-range by the
            // clamp on every step; parse + dispatch.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            let action_opt = buffer
                .parse::<i64>()
                .ok()
                .and_then(|i| action_for_int(setting_key, i));
            state.transition_to_browse();
            match action_opt {
                Some(action) => SettingsKeyOutcome::Action(action),
                None => {
                    tracing::error!(
                        target: "settings",
                        key = setting_key,
                        "Int stepper Enter has no action_for_int arm — registry skew",
                    );
                    SettingsKeyOutcome::Changed
                }
            }
        }
        // Up / k: small step up.
        KeyCode::Up | KeyCode::Char('k') => apply_step(state, step_delta(1, false)),
        // Down / j: small step down.
        KeyCode::Down | KeyCode::Char('j') => apply_step(state, step_delta(-1, false)),
        // Right / l: large step up.
        KeyCode::Right | KeyCode::Char('l') => apply_step(state, step_delta(1, true)),
        // Left / h: large step down.
        KeyCode::Left | KeyCode::Char('h') => apply_step(state, step_delta(-1, true)),
<<<<<<< HEAD
        // `d` in the Int stepper dispatches `OpenResetConfirm` like Browse mode does
        // Close the stepper first so dispatch finds `ActiveModal::Settings` (the dispatch arm panics in debug mode on a non-Settings modal)
        // The stepper otherwise rejects letters, so intercepting `d` collides with nothing
=======
        // `d` in the Int stepper dispatches
        // `OpenResetConfirm` like Browse mode does. Close the
        // stepper first so dispatch finds `ActiveModal::Settings`
        // (the dispatch arm panics in debug mode if it sees a
        // non-Settings modal). The stepper has no `d` semantic
        // otherwise — letters are intentionally rejected — so
        // the interception is safe.
>>>>>>> e3fdf3ed (Merge 2 (#4))
        KeyCode::Char('d') if key.modifiers.is_empty() => {
            state.transition_to_browse();
            SettingsKeyOutcome::Action(Action::OpenResetConfirm { key: setting_key })
        }
<<<<<<< HEAD
        // Everything else (digits, letters, Backspace, Delete, Home, End, Tab) is silently ignored; the stepper is not a text input
=======
        // Everything else (digits, letters, Backspace, Delete,
        // Home, End, Tab, …) is silently ignored — the stepper is
        // not a text input.
>>>>>>> e3fdf3ed (Merge 2 (#4))
        _ => SettingsKeyOutcome::Unchanged,
    }
}

<<<<<<< HEAD
fn update_int_buffer(state: &mut SettingsModalState, new_buffer: String) {
    let SettingsMode::EditingInt { buffer, .. } = &mut state.state.mode else {
        unreachable!("Int update requires EditingInt state");
    };
    *buffer = new_buffer;
}

/// Number of choices for the picker.
/// Handles both `SettingKind::Enum` (static catalog) and `SettingKind::DynamicEnum` (catalog built from the snapshot at picker-open time).
=======
/// Helper: rewrite the EditingValue mode payload with new buffer +
/// cursor + validation. Centralised so future variants don't need
/// to repeat the pattern-construction boilerplate.
fn update_editing_value_buffer(
    state: &mut SettingsModalState,
    buffer: String,
    cursor_byte: usize,
    validation_error: Option<String>,
) {
    let SettingsModalMode::EditingValue { key, .. } = state.mode else {
        // Caller-provided key was lost on mode shift; this is the
        // belt-and-suspenders fallback for a future refactor.
        return;
    };
    state.mode = SettingsModalMode::EditingValue {
        key,
        buffer,
        cursor_byte,
        validation_error,
    };
}

/// Helper: recompute the validation error for the current buffer
/// against the registered validator. Called on every buffer mutation
/// so the inline error indicator stays in sync.
fn recompute_validation(
    kind: &SettingKind,
    buffer: &str,
    state: &SettingsModalState,
) -> Option<String> {
    match kind {
        SettingKind::String { validator, .. } => {
            validate_string(*validator, buffer, &state.pager_snapshot.available_models)
        }
        SettingKind::Int { min, max, .. } => validate_int(buffer, *min, *max),
        _ => None,
    }
}

/// Number of choices for the picker. Handles both
/// `SettingKind::Enum` (static catalog) and `SettingKind::DynamicEnum`
/// (catalog built from the snapshot at picker-open time).
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub(super) fn picker_choices_len(state: &SettingsModalState, key: SettingKey) -> usize {
    state
        .registry
        .find(key)
        .and_then(|m| match &m.kind {
            SettingKind::Enum { choices, .. } => {
                Some(effective_enum_choices(key, choices, &state.pager_snapshot).len())
            }
            SettingKind::DynamicEnum { source, .. } => {
                Some(dynamic_enum_choices(*source, &state.pager_snapshot).len())
            }
            _ => None,
        })
        .unwrap_or(0)
}

<<<<<<< HEAD
/// Canonical value at index `idx` in the picker's choices, or `None` if the key isn't a registered
/// Enum/DynamicEnum or `idx` is out of bounds.
=======
/// Canonical value at index `idx` in the picker's choices, or `None`
/// if the key isn't a registered Enum/DynamicEnum or `idx` is out of
/// bounds.
///
/// Returns `Option<&'static str>` for static `SettingKind::Enum`
/// settings (zero allocation, since each `EnumChoice.canonical` is
/// itself `&'static str`).
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub(super) fn picker_choice_at(
    state: &SettingsModalState,
    key: SettingKey,
    idx: usize,
) -> Option<&'static str> {
    let meta = state.registry.find(key)?;
    let SettingKind::Enum { choices, .. } = &meta.kind else {
        return None;
    };
    effective_enum_choices(key, choices, &state.pager_snapshot)
        .get(idx)
        .map(|c| c.canonical)
}

<<<<<<< HEAD
/// Allocates one `String` per call; the picker calls it on commit, and per Up/Down only when
/// `supports_preview` is true, so the cost is bounded.
=======
/// Owned-string variant of `picker_choice_at` for picker kinds whose
/// canonicals are runtime-built (`SettingKind::DynamicEnum`).
///
/// Returns the canonical at `idx` as an owned `String`. Allocates one
/// `String` per call — the picker calls this on commit + per-Up/Down
/// only when `supports_preview = true`, so the cost is bounded.
///
/// For static `SettingKind::Enum`, this also resolves correctly
/// (clones the `&'static str` into a `String`), giving the picker
/// a single unified read path when the caller doesn't need to
/// distinguish static vs. dynamic.
>>>>>>> e3fdf3ed (Merge 2 (#4))
fn picker_choice_at_owned(
    state: &SettingsModalState,
    key: SettingKey,
    idx: usize,
) -> Option<String> {
    let meta = state.registry.find(key)?;
    match &meta.kind {
        SettingKind::Enum { choices, .. } => {
            effective_enum_choices(key, choices, &state.pager_snapshot)
                .get(idx)
                .map(|c| c.canonical.to_string())
        }
        SettingKind::DynamicEnum { source, .. } => {
            let resolved = dynamic_enum_choices(*source, &state.pager_snapshot);
            resolved.get(idx).map(|c| c.canonical.clone())
        }
        _ => None,
    }
}

<<<<<<< HEAD
/// F2 / Ctrl+, / Cmd+, are the modal-internal close keys. `handle_filter_focused` has its own. EscEsc
/// arm that exits filter mode without closing.
=======
/// F2 / Ctrl+, / Cmd+, are the modal-internal close keys.
///
/// Esc is intentionally NOT matched here: the `ModalWindow` chrome
/// (`views/modal_window.rs:handle_modal_key`) intercepts Esc and
/// returns `ModalWindowOutcome::CloseRequested` before this function
/// sees the event in Browse mode. `handle_filter_focused` has its own
/// Esc arm that exits filter mode without closing. Documented in the
/// module docstring.
>>>>>>> e3fdf3ed (Merge 2 (#4))
fn is_close_key(key: &KeyEvent) -> bool {
    if key.code == KeyCode::F(2) {
        return true;
    }
    if key.code == KeyCode::Char(',')
        && (key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SUPER))
    {
        return true;
    }
    false
}

fn changed_if(b: bool) -> SettingsKeyOutcome {
    if b {
        SettingsKeyOutcome::Changed
    } else {
        SettingsKeyOutcome::Unchanged
    }
}

<<<<<<< HEAD
/// When a sub-pane mouse handler returns `Unchanged` but the breadcrumb hover flipped, upgrade to `Changed` so the renderer repaints the breadcrumb.
/// Non-`Unchanged` outcomes pass through so an `Action` or `Changed` from the inner handler keeps its meaning.
=======
/// Helper for `handle_settings_mouse`: when a sub-pane mouse handler
/// returns `Unchanged` but the breadcrumb hover flipped, upgrade to
/// `Changed` so the renderer repaints the breadcrumb with the new
/// fg color. Non-`Unchanged` outcomes pass through unmodified so an
/// `Action` or `Changed` from the inner handler keeps its meaning.
>>>>>>> e3fdf3ed (Merge 2 (#4))
fn upgrade_if_breadcrumb_flipped(
    outcome: SettingsKeyOutcome,
    breadcrumb_flipped: bool,
) -> SettingsKeyOutcome {
    if breadcrumb_flipped && matches!(outcome, SettingsKeyOutcome::Unchanged) {
        SettingsKeyOutcome::Changed
    } else {
        outcome
    }
}

fn handle_browse(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => changed_if(state.advance_next()),
        KeyCode::Up | KeyCode::Char('k') => changed_if(state.advance_prev()),
        KeyCode::PageDown => {
            let mut moved = false;
            for _ in 0..10 {
                moved |= state.advance_next();
            }
            changed_if(moved)
        }
        KeyCode::PageUp => {
            let mut moved = false;
            for _ in 0..10 {
                moved |= state.advance_prev();
            }
            changed_if(moved)
        }
        KeyCode::Char('g') if key.modifiers.is_empty() => {
<<<<<<< HEAD
            // First selectable row in the filtered set
            // With no filter active, `filtered_cache` is `(0..rows.len())`, so this resolves to the first row
=======
            // First selectable row IN THE FILTERED SET. When no filter
            // is active, `filtered_cache` is `(0..rows.len())` so this
            // resolves to the first row.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            let first = state
                .filtered_cache
                .iter()
                .copied()
<<<<<<< HEAD
                .find(|&idx| {
                    state
                        .rows
                        .get(idx)
                        .is_some_and(|r| matches!(r, RowEntry::Setting { .. }))
                })
=======
                .find(|&idx| matches!(state.rows[idx], RowEntry::Setting { .. }))
>>>>>>> e3fdf3ed (Merge 2 (#4))
                .unwrap_or(state.selected);
            if first != state.selected {
                state.selected = first;
                SettingsKeyOutcome::Changed
            } else {
                SettingsKeyOutcome::Unchanged
            }
        }
        KeyCode::Char('G') => {
<<<<<<< HEAD
            // Last selectable row in the filtered set
=======
            // Last selectable row IN THE FILTERED SET.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            let last = state
                .filtered_cache
                .iter()
                .rev()
                .copied()
<<<<<<< HEAD
                .find(|&idx| {
                    state
                        .rows
                        .get(idx)
                        .is_some_and(|r| matches!(r, RowEntry::Setting { .. }))
                })
=======
                .find(|&idx| matches!(state.rows[idx], RowEntry::Setting { .. }))
>>>>>>> e3fdf3ed (Merge 2 (#4))
                .unwrap_or(state.selected);
            if last != state.selected {
                state.selected = last;
                SettingsKeyOutcome::Changed
            } else {
                SettingsKeyOutcome::Unchanged
            }
        }
<<<<<<< HEAD
        // Right/`l` expands the focused row's description inline; Left/`h` collapses it
        // Expansion is per-row and persists across selection moves; multiple rows can be expanded at once
=======
        // User-feedback follow-up: Right/`l` expands the focused
        // row's description inline; Left/`h` collapses it.
        // Expansion is per-row + persists across selection moves
        // (multiple rows can be expanded simultaneously).
>>>>>>> e3fdf3ed (Merge 2 (#4))
        KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
            if let Some((key, _meta)) = state.focused_setting()
                && state.expanded_keys.insert(key)
            {
                return SettingsKeyOutcome::Changed;
            }
            SettingsKeyOutcome::Unchanged
        }
        KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
            if let Some((key, _meta)) = state.focused_setting()
                && state.expanded_keys.remove(key)
            {
                return SettingsKeyOutcome::Changed;
            }
            SettingsKeyOutcome::Unchanged
        }
<<<<<<< HEAD
        KeyCode::Char(' ') => {
            if let Some(action) = state.toggle_focused_bool() {
                SettingsKeyOutcome::Action(action)
            } else {
                SettingsKeyOutcome::Unchanged
            }
        }
        KeyCode::Enter => {
            // A group row opens its sub-sheet of child toggles
            if state.try_enter_picking_group() {
                return SettingsKeyOutcome::Changed;
            }
            // For Bool, Enter behaves like Space: both keys toggle
            if let Some(action) = state.toggle_focused_bool() {
                return SettingsKeyOutcome::Action(action);
            }
            // An Enum row enters PickingEnum mode; the picker sub-pane takes over rendering and key routing from here
            if state.try_enter_picking_enum() {
                return SettingsKeyOutcome::Changed;
            }
            // A String or Int row enters EditingValue mode; the inline editor takes over rendering and key routing
=======
        KeyCode::Char(' ') | KeyCode::Enter => {
            if state.try_enter_picking_group() {
                return SettingsKeyOutcome::Changed;
            }
            if let Some(action) = state.toggle_focused_bool() {
                return SettingsKeyOutcome::Action(action);
            }
            if state.try_enter_picking_enum() {
                return SettingsKeyOutcome::Changed;
            }
>>>>>>> e3fdf3ed (Merge 2 (#4))
            if state.try_enter_editing_value() {
                return SettingsKeyOutcome::Changed;
            }
            SettingsKeyOutcome::Unchanged
        }
        // `i` aliases `/` (vim-nav "press i to search").
        KeyCode::Char('/') | KeyCode::Char('i') if key.modifiers.is_empty() => {
<<<<<<< HEAD
            state.focus_filter();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Char('d') if key.modifiers.is_empty() => {
            // Reset to default: resolve the focused row's setting key and dispatch `Action::OpenResetConfirm`.
            // Cancel therefore returns to this exact modal state, with filter, scroll, and selection
            // preserved. Headers and unmapped rows are no-ops; `d` only acts on a focused setting row.
=======
            state.mode = SettingsModalMode::FilterFocused;
            SettingsKeyOutcome::Changed
        }
        KeyCode::Char('d') if key.modifiers.is_empty() => {
            // Reset-to-default. Resolves the focused row's
            // setting key and dispatches `Action::OpenResetConfirm`.
            // The dispatch arm boxes the SettingsModalState into the
            // `ActiveModal::ResetSettingsConfirm { settings_state, key, .. }`
            // variant so cancel returns to this exact modal state
            // (filter/scroll/selection preserved). Headers and
            // unmapped rows are no-ops — `d` only acts on a focused
            // setting row.
            //
            // We intentionally do NOT gate this on whether the
            // current value already equals the default — the
            // confirmation dialog gives the user a moment to back
            // out either way, and the dispatch arm shows an
            // "Already at default" toast on idempotent confirm.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            match state.focused_setting() {
                // Group rows have no scalar default to reset.
                Some((_, meta)) if matches!(meta.kind, SettingKind::Group { .. }) => {
                    SettingsKeyOutcome::Unchanged
                }
<<<<<<< HEAD
                // A locked row isn't the user's to change, by `d` any more than by Enter (which `try_enter_picking_enum` refuses)
                // The dispatch-time guard would catch it anyway, but only after a confirm dialog for a change that cannot happen
                Some((key, _meta)) if state.row_lock(key).is_some() => {
                    SettingsKeyOutcome::Unchanged
                }
                Some((key, _meta)) => SettingsKeyOutcome::Action(Action::OpenResetConfirm { key }),
                // The focused row is a header (or out of bounds), so `d` has nothing to reset
=======
                Some((key, _meta)) => SettingsKeyOutcome::Action(Action::OpenResetConfirm { key }),
                // Focused row is a header (or out-of-bounds) — `d`
                // has nothing to reset. Unchanged so the user can
                // still navigate / type / etc.
>>>>>>> e3fdf3ed (Merge 2 (#4))
                None => SettingsKeyOutcome::Unchanged,
            }
        }
        KeyCode::Backspace => {
<<<<<<< HEAD
            // Continue editing a committed query without refocusing the filter.
            if state.query().is_empty() {
                return SettingsKeyOutcome::Unchanged;
            }
            let outcome = state.state.filter.delete_last_grapheme();
            apply_filter_edit(state, outcome)
=======
            // Continue editing the query from Browse mode (the commit
            // path via Enter preserves the query, so Browse can be
            // entered with a non-empty query). Pop one char and
            // re-broaden the filter without switching modes. Mirrors
            // `memory_modal::handle_browse`'s Backspace arm.
            if state.query.pop().is_some() {
                state.query_cursor = state.query.len();
                state.invalidate_filter();
                state.clamp_selected_to_visible();
                SettingsKeyOutcome::Changed
            } else {
                SettingsKeyOutcome::Unchanged
            }
>>>>>>> e3fdf3ed (Merge 2 (#4))
        }
        _ => SettingsKeyOutcome::Unchanged,
    }
}

fn handle_filter_focused(state: &mut SettingsModalState, key: &KeyEvent) -> SettingsKeyOutcome {
    match key.code {
        KeyCode::Esc => {
<<<<<<< HEAD
            if !state.query().is_empty() {
                state.state.filter.reset();
                state.invalidate_filter();
                state.clamp_selected_to_visible();
            }
=======
            state.query.clear();
            state.query_cursor = 0;
            state.invalidate_filter();
            state.clamp_selected_to_visible();
>>>>>>> e3fdf3ed (Merge 2 (#4))
            state.transition_to_browse();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Enter => {
<<<<<<< HEAD
            // Commit the filter: exit FilterFocused and return to Browse, preserving the query
            // The user can then Space/Enter the focused filtered setting at once; clearing here would force re-navigating the full list
=======
            // Commit the filter: exit FilterFocused, return to Browse,
            // PRESERVE the query so the user can immediately Space /
            // Enter to toggle the focused (filtered) setting. This is
            // the standard TUI filter UX (fixes the
            // dead-end where Esc-clear made post-filter toggling
            // impossible without re-navigating the full list).
>>>>>>> e3fdf3ed (Merge 2 (#4))
            state.transition_to_browse();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Down => changed_if(state.advance_next()),
        KeyCode::Up => changed_if(state.advance_prev()),
        KeyCode::PageDown => {
<<<<<<< HEAD
            // Match Browse mode's PageDown: advance 10 rows
=======
            // Match Browse mode's fast-scroll affordance (advance 10).
>>>>>>> e3fdf3ed (Merge 2 (#4))
            let mut moved = false;
            for _ in 0..10 {
                moved |= state.advance_next();
            }
            changed_if(moved)
        }
        KeyCode::PageUp => {
            let mut moved = false;
            for _ in 0..10 {
                moved |= state.advance_prev();
            }
            changed_if(moved)
        }
<<<<<<< HEAD
        KeyCode::Tab => SettingsKeyOutcome::Unchanged,
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
            if !state.query().is_empty() {
                state.state.filter.reset();
                state.invalidate_filter();
                state.clamp_selected_to_visible();
            }
            SettingsKeyOutcome::Changed
        }
        _ => {
            let outcome = state
                .state
                .filter
                .handle_key_with_insert_policy(key, safe_settings_char);
            apply_filter_edit(state, outcome)
        }
    }
}

fn safe_settings_char(character: char) -> bool {
    !crate::render::line_utils::is_unsafe_display_char(character)
}

#[cfg(test)]
pub(super) fn set_filter_cursor(state: &mut SettingsModalState, cursor_byte: usize) {
    let _ = state.state.filter.set_cursor_byte(cursor_byte);
}

fn apply_filter_edit(
    state: &mut SettingsModalState,
    outcome: LineEditOutcome,
) -> SettingsKeyOutcome {
    match outcome {
        LineEditOutcome::TextChanged => {
=======
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
            // Clears entire query (not cursor-to-start) to match picker behavior.
            if state.query.is_empty() {
                return SettingsKeyOutcome::Unchanged;
            }
            state.query.clear();
            state.query_cursor = 0;
>>>>>>> e3fdf3ed (Merge 2 (#4))
            state.invalidate_filter();
            state.clamp_selected_to_visible();
            SettingsKeyOutcome::Changed
        }
<<<<<<< HEAD
        LineEditOutcome::HandledNoChange | LineEditOutcome::CursorChanged => {
            SettingsKeyOutcome::Changed
        }
        LineEditOutcome::Unhandled => SettingsKeyOutcome::Unchanged,
    }
}

/// Handle a mouse event in the modal content area.
=======
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            state.query.insert(state.query_cursor, c);
            state.query_cursor += c.len_utf8();
            state.invalidate_filter();
            state.clamp_selected_to_visible();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Backspace => {
            if state.query_cursor == 0 {
                return SettingsKeyOutcome::Unchanged;
            }
            let prev = state.query[..state.query_cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            state.query.drain(prev..state.query_cursor);
            state.query_cursor = prev;
            state.invalidate_filter();
            state.clamp_selected_to_visible();
            SettingsKeyOutcome::Changed
        }
        KeyCode::Left => {
            if state.query_cursor == 0 {
                return SettingsKeyOutcome::Unchanged;
            }
            state.query_cursor = state.query[..state.query_cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            SettingsKeyOutcome::Changed
        }
        KeyCode::Right => {
            if state.query_cursor >= state.query.len() {
                return SettingsKeyOutcome::Unchanged;
            }
            state.query_cursor = state.query[state.query_cursor..]
                .char_indices()
                .nth(1)
                .map_or(state.query.len(), |(i, _)| state.query_cursor + i);
            SettingsKeyOutcome::Changed
        }
        _ => SettingsKeyOutcome::Unchanged,
    }
}

// ---------------------------------------------------------------------------
// Mouse handling
// ---------------------------------------------------------------------------

/// Handle a mouse event in the modal content area.
///
/// Mirrors `memory_modal::handle_memory_mouse` for parity:
///  - Click on a row selects it; click on a Bool row toggles it.
///  - Click on the `[-]` / `[+]` adornments of an open Int editor
///    steps the value.
///  - Scroll wheel scrolls the row list by ~3 rows per tick.
///
/// **Picker short-circuit:** when the modal is in `PickingEnum`
/// mode, every mouse event is a no-op. `EditingValue` mode handles `[-]` / `[+]`
/// clicks AND treats everything else as a no-op.
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub fn handle_settings_mouse(
    state: &mut SettingsModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> SettingsKeyOutcome {
<<<<<<< HEAD
    // The breadcrumb is hierarchical "up": always return to Browse, never dismiss via `close_on_picker_exit`
    // Clearing the deep-link flag first lets the sub-pane Esc handlers do the preview revert
=======
    // Clicking anywhere on the chrome
    // breadcrumb (the full `Settings › <label>` title) in a
    // sub-pane mode collapses back to Browse. Dispatched FIRST
    // so it wins over the picker / editor mouse handlers (which
    // would otherwise ignore the click as out-of-content). The
    // synthetic Esc is routed through the active sub-pane handler
    // so the same revert-preview / mode-transition logic runs as
    // for keyboard Esc — `handle_picking_enum` reverts the
    // preview action, and `handle_editing_value` just transitions
    // back.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    if matches!(
        kind,
        MouseEventKind::Down(crossterm::event::MouseButton::Left)
    ) && let Some(rect) = state.settings_breadcrumb_rect
        && rect_contains(rect, column, row)
    {
<<<<<<< HEAD
        state.close_on_picker_exit = false;
        let synthetic = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        match state.state.mode_kind() {
            SettingsModeKind::PickingEnum => {
                return handle_picking_enum(state, &synthetic);
            }
            SettingsModeKind::PickingGroup => {
                return handle_picking_group(state, &synthetic);
            }
            SettingsModeKind::EditingString | SettingsModeKind::EditingInt => {
=======
        let synthetic = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        match state.mode {
            SettingsModalMode::PickingEnum { .. } => {
                return handle_picking_enum(state, &synthetic);
            }
            SettingsModalMode::PickingGroup { .. } => {
                return handle_picking_group(state, &synthetic);
            }
            SettingsModalMode::EditingValue { .. } => {
>>>>>>> e3fdf3ed (Merge 2 (#4))
                return handle_editing_value(state, &synthetic);
            }
            _ => {}
        }
    }

<<<<<<< HEAD
    // Track hover for the breadcrumb hit-rect so the renderer can repaint the title with the brighter `accent_user` fg when the mouse is over it
    // Without this cue the breadcrumb looks no different from the rest of the modal title
    // Tracked here so a hover transition registers even when the row-list, picker, or editor mouse handlers below short-circuit on the Moved event
=======
    // Track hover state for the
    // breadcrumb hit-rect so the renderer can repaint the title
    // with the brighter `accent_user` fg when the user's mouse
    // is over it. Without this cue the breadcrumb is visually
    // indistinguishable from the rest of the modal title chrome.
    // Tracked here so a hover transition is registered even when
    // the row-list / picker / editor mouse handlers below short-
    // circuit on the Moved event.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    let breadcrumb_hover_flipped = if matches!(kind, MouseEventKind::Moved) {
        let now_hovered = state
            .settings_breadcrumb_rect
            .map(|r| rect_contains(r, column, row))
            .unwrap_or(false);
        let flipped = now_hovered != state.breadcrumb_hovered;
        state.breadcrumb_hovered = now_hovered;
<<<<<<< HEAD
        // In sub-pane modes the mouse handlers don't update hover_row, so a flipped breadcrumb hover is the only thing that could redraw
        // In Browse the row-list handler below already returns Changed when hover_row moves, so it runs as usual
=======
        // In sub-pane modes the row-list / picker / editor mouse
        // handlers don't update hover_row, so a flipped breadcrumb
        // hover is the only thing that could redraw. Force a
        // Changed outcome for those modes; in Browse the row-list
        // handler below already returns Changed when hover_row
        // moves so we let it run.
>>>>>>> e3fdf3ed (Merge 2 (#4))
        flipped
    } else {
        false
    };

<<<<<<< HEAD
    // Handle `[-]` / `[+]` clicks when in EditingValue mode and the row is an Int
    // All other events in EditingValue (scrolls, off-adornment clicks) are no-ops
    if matches!(
        state.state.mode_kind(),
        SettingsModeKind::EditingString | SettingsModeKind::EditingInt
    ) {
=======
    // Handle `[-]` / `[+]` clicks
    // when in EditingValue mode AND the row is an Int. All other
    // events in EditingValue (scrolls, off-adornment clicks) are
    // no-ops.
    if matches!(state.mode, SettingsModalMode::EditingValue { .. }) {
>>>>>>> e3fdf3ed (Merge 2 (#4))
        let outcome = handle_editor_mouse(state, kind, column, row);
        return upgrade_if_breadcrumb_flipped(outcome, breadcrumb_hover_flipped);
    }

<<<<<<< HEAD
    // PickingEnum: click-to-pick on choice rects; the scroll wheel is a no-op (the picker is bounded; scrolling there could surprise)
    if state.state.mode_kind() == SettingsModeKind::PickingEnum {
=======
    // PickingEnum: click-to-pick on choice rects, scroll wheel is a
    // no-op (the picker is bounded; scroll there could surprise).
    if matches!(state.mode, SettingsModalMode::PickingEnum { .. }) {
>>>>>>> e3fdf3ed (Merge 2 (#4))
        let outcome = handle_picker_mouse(state, kind, column, row);
        return upgrade_if_breadcrumb_flipped(outcome, breadcrumb_hover_flipped);
    }

<<<<<<< HEAD
    // PickingGroup: hover tracks the child rects; a click toggles the clicked child in place
    // As in the enum picker, the viewport is bounded and scroll is a no-op
    if state.state.mode_kind() == SettingsModeKind::PickingGroup {
=======
    // PickingGroup: hover tracks the child rects; a click toggles the clicked
    // child in place (same bounded-viewport, scroll-is-a-no-op contract).
    if matches!(state.mode, SettingsModalMode::PickingGroup { .. }) {
>>>>>>> e3fdf3ed (Merge 2 (#4))
        let outcome = handle_group_mouse(state, kind, column, row);
        return upgrade_if_breadcrumb_flipped(outcome, breadcrumb_hover_flipped);
    }

    let on_list = rect_contains(state.list_area, column, row);

    // Mouse hover highlight (parity with scrollback).
<<<<<<< HEAD
    // Walk `state.row_rects` to find the row under the cursor and update `state.hover_row`
    // Return early so later arms don't repeat the find; clicks and scrolls fall through to the arms below
=======
    // Walks `state.row_rects` to find the row under the cursor and
    // updates `state.hover_row`. Returns early so subsequent arms
    // don't need to repeat the find. Clicks and scrolls fall
    // through to the existing arms below.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    if matches!(kind, MouseEventKind::Moved) {
        let new_hover = state
            .row_rects
            .iter()
            .position(|r| rect_contains(*r, column, row))
            .filter(|&idx| matches!(state.rows.get(idx), Some(RowEntry::Setting { .. })));
        if new_hover != state.hover_row {
            state.hover_row = new_hover;
            return SettingsKeyOutcome::Changed;
        }
        return SettingsKeyOutcome::Unchanged;
    }

    match kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if !on_list {
                return SettingsKeyOutcome::Unchanged;
            }
            // Resolve the clicked row.
            let clicked_idx = state
                .row_rects
                .iter()
                .position(|r| rect_contains(*r, column, row));
            let Some(idx) = clicked_idx else {
                return SettingsKeyOutcome::Unchanged;
            };
            // Clicking a header is a no-op.
<<<<<<< HEAD
            if !state
                .rows
                .get(idx)
                .is_some_and(|r| matches!(r, RowEntry::Setting { .. }))
            {
                return SettingsKeyOutcome::Unchanged;
            }
            // Two-stage clicks. Click on a different row: only select, so the user can read the description
            // first.
            let Some(&row_rect) = state.row_rects.get(idx) else {
                return SettingsKeyOutcome::Unchanged;
            };
            // Col 0 of the row is the `▸`/`▾` triangle glyph. A click there toggles expansion without touching
            // the value, matching the keyboard's Right/Left arrows. Two-line rows have `row_rect.height = 2`
            // with the triangle on line 1 only.
            let on_triangle = column == row_rect.x && row == row_rect.y;
            // The 5-col indicator hit-rect sits on the value column on the right, not the left edge.
=======
            if !matches!(state.rows[idx], RowEntry::Setting { .. }) {
                return SettingsKeyOutcome::Unchanged;
            }
            // Two-stage click semantics:
            //   - Click on a different row: only select (lets the user
            //     read the description first).
            //   - Click on the already-selected Bool row: toggle.
            //   - Click on the already-selected Enum row: open picker.
            //   - Click on the indicator cells of any Bool / Enum /
            //     String / Int row: select + activate in one click
            //     (Fitts's-law nudge — 5-col hit-rect around the
            //     small glyph).
            //
            // The per-kind dispatch is
            // collapsed into a single `if` chain — the
            // `toggle_focused_bool` / `try_enter_picking_enum` /
            // `try_enter_editing_value` helpers all return falsy
            // for non-matching kinds, so the per-kind predicates
            // are redundant.
            let row_rect = state.row_rects[idx];
            // User-feedback follow-up: col 0 of the row is the
            // `▸`/`▾` triangle glyph. A click there toggles
            // expansion (no value mutation) — matching the
            // keyboard's Right/Left arrow contract. The triangle
            // is at exactly column `row_rect.x` and is 1 cell wide.
            //
            // Two-line rows have `row_rect.height = 2`,
            // and the triangle stays on LINE 1 only — clicks on
            // line 2's col 0 (which is empty padding) shouldn't
            // toggle expansion. The y-check enforces this.
            let on_triangle = column == row_rect.x && row == row_rect.y;
            // The 5-col Fitts's-law
            // indicator hit-rect sits on the
            // value column on the right (rather than the left edge). Clicking the Bool's
            // `on`/`off` text toggles the bool in one click; clicking
            // an Enum/String/DynamicEnum/Int value opens the
            // picker/editor in one click. The hit-rect is supplied
            // by `render_setting_row` via
            // `state.value_hit_rects[idx]`.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            let value_rect = state.value_hit_rects.get(idx).copied().unwrap_or_default();
            let on_value = rect_contains(value_rect, column, row);
            let was_selected_already = state.selected == idx;
            let _ = state.select_at(idx);

            if on_triangle && let Some((key, _meta)) = state.focused_setting() {
<<<<<<< HEAD
                // Toggle expansion, mirroring the keyboard Right/Left arrows
=======
                // Toggle expansion. Mirrors the keyboard
                // Right/Left arrow contract.
>>>>>>> e3fdf3ed (Merge 2 (#4))
                if state.expanded_keys.contains(key) {
                    state.expanded_keys.remove(key);
                } else {
                    state.expanded_keys.insert(key);
                }
                return SettingsKeyOutcome::Changed;
            }

            if on_value || was_selected_already {
                if state.try_enter_picking_group() {
                    return SettingsKeyOutcome::Changed;
                }
                if let Some(action) = state.toggle_focused_bool() {
                    return SettingsKeyOutcome::Action(action);
                }
                if state.try_enter_picking_enum() || state.try_enter_editing_value() {
                    return SettingsKeyOutcome::Changed;
                }
            }
<<<<<<< HEAD
            // Selection moved (or was already on this row); the re-render reflects the new focus
=======
            // Selection moved (or was already on this row); the
            // re-render reflects the new focus.
>>>>>>> e3fdf3ed (Merge 2 (#4))
            SettingsKeyOutcome::Changed
        }
        MouseEventKind::ScrollDown => {
            if !on_list {
                return SettingsKeyOutcome::Unchanged;
            }
            let mut moved = false;
            for _ in 0..3 {
                moved |= state.advance_next();
            }
            changed_if(moved)
        }
        MouseEventKind::ScrollUp => {
            if !on_list {
                return SettingsKeyOutcome::Unchanged;
            }
            let mut moved = false;
            for _ in 0..3 {
                moved |= state.advance_prev();
            }
            changed_if(moved)
        }
        _ => SettingsKeyOutcome::Unchanged,
    }
}

/// Handle a mouse event while the modal is in `PickingEnum` mode.
<<<<<<< HEAD
=======
///
/// Left-click on any line of a choice's multi-line hit-rect moves
/// the picker focus to that choice (and fires the matching preview
/// dispatch, mirroring keyboard Up/Down). Clicks outside any choice
/// rect are no-ops, as are scroll wheel events (the picker viewport
/// is bounded; in-picker scrolling could surprise).
///
/// Continuation lines of a word-wrapped description share the same
/// hit-rect as the choice they belong to — clicking the second line
/// of "Opt out" picks "Opt out", same as clicking its symbol.
>>>>>>> e3fdf3ed (Merge 2 (#4))
fn handle_picker_mouse(
    state: &mut SettingsModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> SettingsKeyOutcome {
<<<<<<< HEAD
    // Hover highlight for picker choices
    // Track the choice index under the cursor in `state.hover_row` (the same field the row-list path uses; it is mode-aware)
=======
    // Hover highlight for picker choices. Tracks the
    // choice index under the cursor in `state.hover_row` (same
    // field as the row-list path; the field is mode-aware via the
    // active `state.mode`).
>>>>>>> e3fdf3ed (Merge 2 (#4))
    if matches!(kind, MouseEventKind::Moved) {
        let new_hover = state
            .picker_choice_rects
            .iter()
            .position(|r| r.height > 0 && rect_contains(*r, column, row));
        if new_hover != state.hover_row {
            state.hover_row = new_hover;
            return SettingsKeyOutcome::Changed;
        }
        return SettingsKeyOutcome::Unchanged;
    }

    let MouseEventKind::Down(crossterm::event::MouseButton::Left) = kind else {
        return SettingsKeyOutcome::Unchanged;
    };
<<<<<<< HEAD
    // Snapshot the picker payload before mutating the state.
    let (setting_key, current_idx, original_value, supports_preview) = match &state.state.mode {
        SettingsMode::PickingEnum {
=======
    // Snapshot the picker payload under the immutable borrow.
    let (setting_key, current_idx, original_value, supports_preview) = match &state.mode {
        SettingsModalMode::PickingEnum {
>>>>>>> e3fdf3ed (Merge 2 (#4))
            key,
            choices_idx,
            original_value,
            supports_preview,
        } => (
            *key,
            *choices_idx,
            original_value.clone(),
            *supports_preview,
        ),
<<<<<<< HEAD
        _ => unreachable!("picker mouse handler requires PickingEnum state"),
=======
        _ => return SettingsKeyOutcome::Unchanged,
>>>>>>> e3fdf3ed (Merge 2 (#4))
    };
    let clicked_idx = state
        .picker_choice_rects
        .iter()
        .position(|r| r.height > 0 && rect_contains(*r, column, row));
    let Some(target_idx) = clicked_idx else {
<<<<<<< HEAD
        state.picker_last_click = None;
        return SettingsKeyOutcome::Unchanged;
    };
    // Do not move focus and then Enter-commit: that drops the preview Action
    // and can persist a different radio than the one clicked.
    if picker_click_is_double(state, target_idx) && target_idx == current_idx {
        state.picker_last_click = None;
        let synthetic = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        return handle_picking_enum(state, &synthetic);
    }
    let outcome = if target_idx == current_idx {
        SettingsKeyOutcome::Unchanged
    } else {
        set_picker_idx(
            state,
            setting_key,
            target_idx,
            original_value,
            supports_preview,
        )
    };
    // Arm after `set_picker_idx`, which clears a pending double-click.
    state.picker_last_click = Some((target_idx, std::time::Instant::now()));
    outcome
}

fn picker_click_is_double(state: &SettingsModalState, idx: usize) -> bool {
    state.picker_last_click.is_some_and(|(prev, at)| {
        prev == idx && at.elapsed().as_millis() < crate::app::agent_view::MULTI_CLICK_TIMEOUT_MS
    })
}

/// Handle a mouse event while the modal is in `PickingGroup` mode.
=======
        return SettingsKeyOutcome::Unchanged;
    };
    if target_idx == current_idx {
        // Already focused — re-clicking the same choice is a no-op
        // (kept for parity with the row-list's "already-focused
        // click commits" semantics; commit fires on Enter, not on
        // a re-click).
        return SettingsKeyOutcome::Unchanged;
    }
    // Reuse the keyboard nav helper to update `choices_idx` AND
    // fire the matching preview Action (when the kind supports it).
    set_picker_idx(
        state,
        setting_key,
        target_idx,
        original_value,
        supports_preview,
    )
}

/// Handle a mouse event while the modal is in `PickingGroup` mode.
///
/// Hover tracks the child row under the cursor; a left-click moves focus to the
/// clicked child AND toggles it in one click (toggles, unlike the enum picker's
/// commit-on-Enter). Scroll wheel is a no-op (the sub-sheet is bounded).
>>>>>>> e3fdf3ed (Merge 2 (#4))
fn handle_group_mouse(
    state: &mut SettingsModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> SettingsKeyOutcome {
    if matches!(kind, MouseEventKind::Moved) {
        let new_hover = state
            .picker_choice_rects
            .iter()
            .position(|r| r.height > 0 && rect_contains(*r, column, row));
        if new_hover != state.hover_row {
            state.hover_row = new_hover;
            return SettingsKeyOutcome::Changed;
        }
        return SettingsKeyOutcome::Unchanged;
    }
    let MouseEventKind::Down(crossterm::event::MouseButton::Left) = kind else {
        return SettingsKeyOutcome::Unchanged;
    };
<<<<<<< HEAD
    let group_key = match &state.state.mode {
        SettingsMode::PickingGroup { key, .. } => *key,
        _ => unreachable!("group mouse handler requires PickingGroup state"),
=======
    let group_key = match &state.mode {
        SettingsModalMode::PickingGroup { key, .. } => *key,
        _ => return SettingsKeyOutcome::Unchanged,
>>>>>>> e3fdf3ed (Merge 2 (#4))
    };
    let children = group_children(state, group_key);
    let clicked_idx = state
        .picker_choice_rects
        .iter()
        .position(|r| r.height > 0 && rect_contains(*r, column, row));
    let Some(idx) = clicked_idx else {
        return SettingsKeyOutcome::Unchanged;
    };
<<<<<<< HEAD
    state.transition_to_picking_group(group_key, idx);
=======
    state.mode = SettingsModalMode::PickingGroup {
        key: group_key,
        child_idx: idx,
    };
>>>>>>> e3fdf3ed (Merge 2 (#4))
    let Some(child_key) = children.get(idx).copied() else {
        return SettingsKeyOutcome::Changed;
    };
    let cur = matches!(state.value_for(child_key), Some(SettingValue::Bool(true)));
    match action_for_bool(child_key, !cur) {
        Some(action) => SettingsKeyOutcome::Action(action),
        None => SettingsKeyOutcome::Changed,
    }
}

<<<<<<< HEAD
/// Handle a mouse event while in `EditingValue` mode.
/// Clicks on the Int editor's `[-]` / `[+]` adornments dispatch as keyboard-equivalent Down/Up steps; everything else is a no-op.
=======
/// Handle a mouse event while in `EditingValue` mode. Dispatches
/// clicks on the Int editor's `[-]` / `[+]` adornments as
/// keyboard-equivalent Down/Up steps; everything else is a no-op.
>>>>>>> e3fdf3ed (Merge 2 (#4))
fn handle_editor_mouse(
    state: &mut SettingsModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> SettingsKeyOutcome {
    let MouseEventKind::Down(crossterm::event::MouseButton::Left) = kind else {
        return SettingsKeyOutcome::Unchanged;
    };
    let (dec_rect, inc_rect) = state.editor_adornment_rects;
    let step_dir = if rect_contains(dec_rect, column, row) {
        StepDir::Down
    } else if rect_contains(inc_rect, column, row) {
        StepDir::Up
    } else {
        return SettingsKeyOutcome::Unchanged;
    };
<<<<<<< HEAD
    // Synthesize the equivalent keyboard event so the step, clamp, and validation logic lives in one place (`handle_editing_value`)
    // Mirrors the picker's choice-click approach
=======
    // Synthesize the equivalent keyboard event so the actual
    // step + clamp + validation logic lives in one place
    // (`handle_editing_value`). Mirrors the picker's choice-click
    // approach.
>>>>>>> e3fdf3ed (Merge 2 (#4))
    let synthetic = KeyEvent::new(
        match step_dir {
            StepDir::Up => KeyCode::Up,
            StepDir::Down => KeyCode::Down,
        },
        KeyModifiers::NONE,
    );
    handle_editing_value(state, &synthetic)
}

<<<<<<< HEAD
/// Direction tag for turning the Int editor's spinner clicks into key events; internal to `handle_editor_mouse`.
=======
/// Direction tag for the Int editor's spinner mouse-click → key
/// synthesis. Internal to `handle_editor_mouse`.
>>>>>>> e3fdf3ed (Merge 2 (#4))
#[derive(Clone, Copy)]
enum StepDir {
    Up,
    Down,
}

fn rect_contains(r: Rect, column: u16, row: u16) -> bool {
    r.width > 0
        && r.height > 0
        && column >= r.x
        && column < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
}
<<<<<<< HEAD
=======

// ---------------------------------------------------------------------------
>>>>>>> e3fdf3ed (Merge 2 (#4))
