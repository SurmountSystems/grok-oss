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
