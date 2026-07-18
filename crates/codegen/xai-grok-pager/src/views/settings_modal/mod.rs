<<<<<<< HEAD
//! Settings modal: opens via F2, `/settings`, command palette, and shortcuts-help.
=======
//! Settings modal — opens via F2, `/settings`, command palette, and
//! shortcuts-help.
>>>>>>> e3fdf3ed (Merge 2 (#4))
//!
//! ## State machine
//!
//! `SettingsModalState` carries a `UiConfig` snapshot plus a mode machine:
//!
<<<<<<< HEAD
//! - `Browse`: j/k navigates rows; Space toggles Bool; Enter opens a chooser/editor for Enum/String/Int.
//! - `FilterFocused`: `/` enters filter mode; `invalidate_filter` recomputes `filtered_cache` on every mutation.
//! - `PickingEnum { ... }`: enum chooser sub-pane.
//! - `EditingValue { ... }`: inline string/int editor.
//!
//! ## Keyboard and mouse parity
=======
//! - `Browse` — j/k navigates rows; Space toggles Bool; Enter opens
//!   a chooser/editor for Enum/String/Int.
//! - `FilterFocused` — `/` enters filter mode; `invalidate_filter`
//!   recomputes `filtered_cache` on every mutation.
//! - `PickingEnum { ... }` — enum chooser sub-pane.
//! - `EditingValue { ... }` — inline string/int editor.
//!
//! ## Keyboard ↔ mouse parity
>>>>>>> e3fdf3ed (Merge 2 (#4))
//!
//! Every keyboard interaction has a mouse equivalent via `handle_mouse`.
//!
//! ## Close-key interception
//!
//! F2/Ctrl+,/Cmd+, are intercepted before mode-specific routing.
<<<<<<< HEAD
//! Esc-in-Browse is handled by the `ModalWindow` chrome (so `is_close_key` does NOT match Esc).
//! Esc-in-FilterFocused exits filter mode without closing.
=======
//! Esc-in-Browse is handled by the `ModalWindow` chrome (so
//! `is_close_key` does NOT match Esc); Esc-in-FilterFocused exits
//! filter mode without closing.
>>>>>>> e3fdf3ed (Merge 2 (#4))

mod input;
mod render;
mod state;

#[cfg(test)]
mod tests;

<<<<<<< HEAD
pub use input::{handle_settings_key, handle_settings_mouse, handle_settings_paste};
pub use render::{ResetConfirmOverlay, render_settings_modal};
#[allow(unused_imports)] // re-export for crate path; used by settings/registry tests
pub(crate) use state::{MAX_PICKER_CHOICES, RowVisibility};
=======
pub use input::{handle_settings_key, handle_settings_mouse};
pub use render::{ResetConfirmOverlay, render_settings_modal};
#[allow(unused_imports)] // re-export for crate path; used by settings/registry tests
pub(crate) use state::MAX_PICKER_CHOICES;
>>>>>>> e3fdf3ed (Merge 2 (#4))
pub use state::{MODAL_TITLE, RowEntry, SettingsKeyOutcome, SettingsModalMode, SettingsModalState};
