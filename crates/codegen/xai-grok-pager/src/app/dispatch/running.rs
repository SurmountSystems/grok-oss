//! `/running`: list live grok-oss TUI windows. Not the Agent Dashboard.

use crate::app::actions::Effect;
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::block::RenderBlock;

/// Commit a refresh-on-open transcript table of live grok-oss windows.
pub(super) fn dispatch_show_running_sessions(app: &mut AppView) -> Vec<Effect> {
    if let ActiveView::Agent(id) = app.active_view
        && let Some(agent) = app.agents.get_mut(&id)
    {
        let text = crate::running_sessions::slash_report();
        agent.scrollback.push_block(RenderBlock::system(text));
    }
    vec![]
}
