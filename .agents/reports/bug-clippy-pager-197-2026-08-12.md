# Clippy fix: xai-grok-pager (-D warnings)

Date: 2026-08-12
Package: `xai-grok-pager`
Scope: listed lib lints plus extras surfaced under `--all-targets`

## Commands / exit codes

| Command | Exit |
|---------|------|
| `cargo fmt -p xai-grok-pager` | 0 |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | **0** |
| `cargo clippy -p xai-grok-pager --all-targets -- -D warnings` | **0** (after extras) |

## Files changed

### Listed (18 sites)

| File | Lint / fix |
|------|------------|
| `src/app/agent_view/mod.rs` | `collapsible_match`: WebFetch arm guard `if !wf.url.is_empty()` |
| `src/app/dispatch/notes.rs` | `collapsible_match`: ToolCall arm guard `if file_paths.len() < 20` |
| `src/app/dispatch/rewind.rs` | `sort_by` → `sort_by_key(\|b\| Reverse(b.prompt_index))` |
| `src/app/dispatch/session/load.rs` | drop redundant `&` on `session_id` in `format!` |
| `src/project_picker/sources.rs` | `sort_by_key(\|b\| Reverse(b.1))` |
| `src/recent_dirs.rs` | `sort_by_key(\|b\| Reverse(b.1))` |
| `src/scrollback/render.rs` | `explicit_counter_loop`: zip `enumerate().skip(..).zip(first_visible_content_y..)` |
| `src/slash/mru.rs` | `sort_by_key(\|b\| Reverse(b.1))` |
| `src/tool_usage.rs` | `sort_by_key(\|a\| Reverse(a.1.count))` |
| `src/views/dashboard/state.rs` | `sort_by_key(\|a\| a.label.to_lowercase())` |
| `src/views/history_search.rs` | `sort_unstable_by_key` Reverse; `index.filter(\|&i\| …)` |
| `src/views/prompt_widget/mod.rs` | two `index.filter(\|&i\| …)` |
| `src/views/tasks_pane.rs` | drop redundant `&` on `prompt_preview` / `suffix` |
| `src/views/welcome/menu.rs` | `explicit_counter_loop`: `enumerate().zip(menu_centered.y..)` |

### Extras from `--all-targets`

| File | Lint / fix |
|------|------------|
| `src/bin/scrollback_search_playground.rs` | `collapsible_match`: `Event::Key(key) if handle_key(...)` |
| `tests/settings_e2e.rs` | `unnecessary_min_or_max`: drop no-op `.max(0)` after `saturating_sub(1)` |

## Notes

- Counter loops keep start/step semantics: screen_y from `first_visible_content_y`, menu y from `menu_centered.y`, early break when past max/height.
- No git add/commit.
- Surgical style-only; no behavior intent change.
