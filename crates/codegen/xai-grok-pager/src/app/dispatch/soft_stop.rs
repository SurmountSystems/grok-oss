//! Soft stop dispatch: arm / disarm / release queue hold.

use crate::app::actions::Effect;
use crate::app::app_view::AppView;

/// Toggle soft stop (Ctrl+Shift+S). Does not cancel mid-turn work.
pub(super) fn dispatch_toggle_soft_stop(app: &mut AppView) -> Vec<Effect> {
    let was_holding = app.soft_stop.is_holding();
    let (_phase, toast) = app.soft_stop.toggle();
    app.show_toast(&toast);
    let mut effects = Vec::new();
    // Releasing a hold: allow queues to drain again.
    if was_holding && app.soft_stop.is_off() {
        let ids: Vec<_> = app.agents.keys().copied().collect();
        for id in ids {
            effects.extend(super::queue::maybe_drain_queue_and_note_peek(app, id));
        }
    }
    effects
}
