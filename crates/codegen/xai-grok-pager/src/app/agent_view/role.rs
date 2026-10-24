//! What an [`AgentView`] is: the root of its session, or a child mirrored from a subagent.
//! The role is set once, by [`AgentView::insert_subagent_view`]; every child-specific read site
//! derives its answer from it here instead of keeping a second flag that could drift.

use crate::app::actions::Effect;
use crate::app::agent::AgentState;
use crate::app::agent_view::AgentView;
use agent_client_protocol as acp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentRole {
    Root,
    Child(ChildLink),
}

/// How a child view relates to the session that spawned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildLink {
    /// The parent session id from the wire; the only session a message to this child may be sent on.
    parent_session_id: acp::SessionId,
    messaging: ChildMessaging,
}

impl ChildLink {
    /// A child with no `agentAddress` on the wire (replayed spawn, workflow child): nothing can be sent to it.
    pub(crate) fn unaddressable(parent_session_id: acp::SessionId) -> Self {
        Self {
            parent_session_id,
            messaging: ChildMessaging::Unaddressable,
        }
    }

    /// An L2 coordinator. The Operator addresses it from the floor prompt.
    pub(crate) fn l2_coordinator(parent_session_id: acp::SessionId) -> Self {
        Self {
            parent_session_id,
            messaging: ChildMessaging::L2Coordinator,
        }
    }

    pub(crate) fn parent_session_id(&self) -> &acp::SessionId {
        &self.parent_session_id
    }
}

/// Whether the pager can address the child at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChildMessaging {
    Unaddressable,
    /// An L2 coordinator the Operator can address from the floor prompt.
    L2Coordinator,
}

/// Which chrome the view paints: its own session's, or the framed takeover a parent draws around a child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewSurface {
    Root,
    ChildTakeover,
}

/// Where the composer's text would go, which decides whether it renders at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerRoute {
    RootSession,
    /// Floor prompt on an open L2 coordinator. Enter sends to that L2.
    L2Floor,
    /// No route exists, so the composer gets zero rows and can't take focus.
    Hidden,
}

impl AgentView {
    /// `None` for the root of a session.
    pub(crate) fn child_link(&self) -> Option<&ChildLink> {
        match &self.role {
            AgentRole::Root => None,
            AgentRole::Child(link) => Some(link),
        }
    }

    pub(crate) fn surface(&self) -> ViewSurface {
        match &self.role {
            AgentRole::Root => ViewSurface::Root,
            AgentRole::Child(_) => ViewSurface::ChildTakeover,
        }
    }

    /// GitHub #232: L1 root execute that can block the Operator box
    /// auto-backgrounds. None when this view is not that case.
    pub(crate) fn l1_blocking_execute_auto_background_effect(&self) -> Option<Effect> {
        if self.surface() != ViewSurface::Root {
            return None;
        }
        if self.session.loading_replay {
            return None;
        }
        if !matches!(
            self.session.state,
            AgentState::TurnRunning | AgentState::CommandRunning { .. }
        ) {
            return None;
        }
        let session_id = self.session.session_id.clone()?;
        let tool_call_id = self
            .session
            .tracker
            .running_execute_tool_call_id()?
            .to_string();
        if self.l1_auto_background_tool_call_id.as_deref() == Some(tool_call_id.as_str()) {
            return None;
        }
        tracing::info!(tool_call_id = %tool_call_id, "Auto-backgrounding L1 blocking execute");
        Some(Effect::DemoteToBackground {
            session_id,
            tool_call_id,
        })
    }

    /// GitHub #243: L1 blocked on a foreground nested agent auto-backgrounds.
    /// send-to-background for that subagent is kept as Ctrl+B; this is the
    /// automatic path so the Operator box is not stuck on a nested wait.
    pub(crate) fn l1_blocking_subagent_auto_background_effect(&self) -> Option<Effect> {
        if self.surface() != ViewSurface::Root {
            return None;
        }
        if self.session.loading_replay {
            return None;
        }
        if !matches!(
            self.session.state,
            AgentState::TurnRunning | AgentState::CommandRunning { .. }
        ) {
            return None;
        }
        let session_id = self.session.session_id.clone()?;
        let tool_call_id = self.blocking_subagent_tool_call_id()?;
        if self.l1_auto_background_tool_call_id.as_deref() == Some(tool_call_id.as_str()) {
            return None;
        }
        tracing::info!(tool_call_id = %tool_call_id, "Auto-backgrounding L1 blocking subagent");
        Some(Effect::DemoteToBackground {
            session_id,
            tool_call_id,
        })
    }

    fn blocking_subagent_tool_call_id(&self) -> Option<String> {
        self.session
            .tracker
            .blocking_subagent_tool_call_id()
            .map(str::to_owned)
            .or_else(|| {
                self.subagent_sessions
                    .values()
                    .find(|info| {
                        info.is_running()
                            && !info.attempt.is_background
                            && info.attempt.workflow_run_id.is_none()
                    })
                    .map(|info| info.subagent_id.to_string())
            })
    }

    /// Ctrl+B / send-to-background is a real product action for a blocking
    /// subagent, and still demotes a root execute. GitHub #243: do not delete
    /// that capability as a stand-in for auto-background.
    pub(crate) fn can_send_to_background(&self) -> bool {
        if self
            .session
            .tracker
            .running_execute_tool_call_id()
            .is_some()
            && self.surface() == ViewSurface::Root
        {
            return true;
        }
        if self.surface() == ViewSurface::ChildTakeover && self.session.state.is_busy() {
            return true;
        }
        if self.surface() != ViewSurface::Root {
            return false;
        }
        self.active_subagent.is_some()
            || self
                .session
                .tracker
                .blocking_subagent_tool_call_id()
                .is_some()
            || self.has_running_foreground_subagent()
    }

    /// Apply L1 auto-background for a blocking execute or a blocking nested
    /// agent. Records the tool-call id so the same wait does not re-fire.
    pub(crate) fn take_l1_blocking_auto_background(&mut self) -> Option<Effect> {
        if let Some(effect) = self.l1_blocking_execute_auto_background_effect() {
            if let Effect::DemoteToBackground { tool_call_id, .. } = &effect {
                self.l1_auto_background_tool_call_id = Some(tool_call_id.clone());
            }
            return Some(effect);
        }
        self.take_l1_blocking_subagent_auto_background()
    }

    fn take_l1_blocking_subagent_auto_background(&mut self) -> Option<Effect> {
        let effect = self.l1_blocking_subagent_auto_background_effect()?;
        self.apply_blocking_subagent_background(&effect);
        Some(effect)
    }

    /// GitHub #243: send a blocking nested agent to the background. On an
    /// open takeover this also closes the overlay so L1 can type. This is
    /// the Ctrl+B path; auto-background does not close the overlay.
    pub(crate) fn take_send_blocking_subagent_to_background(&mut self) -> Option<Effect> {
        let session_id = self.session.session_id.clone()?;
        let Some(child_sid) = self.active_subagent.clone() else {
            return self.take_l1_blocking_subagent_auto_background();
        };
        let tool_call_id = self
            .subagent_sessions
            .iter()
            .find(|(key, info)| {
                *key == &child_sid || info.child_session_id.as_ref() == child_sid.as_str()
            })
            .map(|(_, info)| info.subagent_id.to_string())
            .unwrap_or_else(|| child_sid.clone());
        let effect = Effect::DemoteToBackground {
            session_id,
            tool_call_id,
        };
        self.apply_blocking_subagent_background(&effect);
        self.close_subagent_fullscreen();
        Some(effect)
    }

    fn apply_blocking_subagent_background(&mut self, effect: &Effect) {
        let Effect::DemoteToBackground { tool_call_id, .. } = effect else {
            return;
        };
        for info in self.subagent_sessions.values_mut() {
            if info.is_running()
                && !info.attempt.is_background
                && info.attempt.workflow_run_id.is_none()
            {
                info.attempt.is_background = true;
            }
        }
        let wait_keys = self.session.tracker.blocking_subagent_wait_keys();
        self.session.tracker.remove_blocking_waits(&wait_keys);
        self.l1_auto_background_tool_call_id = Some(tool_call_id.clone());
    }

    pub(crate) fn composer_route(&self) -> ComposerRoute {
        match &self.role {
            AgentRole::Root => ComposerRoute::RootSession,
            AgentRole::Child(ChildLink {
                messaging: ChildMessaging::L2Coordinator,
                ..
            }) => ComposerRoute::L2Floor,
            AgentRole::Child(ChildLink {
                messaging: ChildMessaging::Unaddressable,
                ..
            }) => ComposerRoute::Hidden,
        }
    }
}

#[cfg(test)]
#[path = "role_tests.rs"]
mod tests;
