//! Inline scrollback card for a max-tier credit-limit exhaustion.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::appearance::AppearanceConfig;
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{AccentStyle, BlockContext, BlockLine, BlockOutput};
use crate::theme::Theme;

/// What the max-tier card offers besides the heading and the billing URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditLimitCardAction {
    /// Buy more credits.
    PurchaseCredits,
    /// Raise an existing pay-as-you-go spending cap.
    IncreasePaygLimit,
    /// Turn pay-as-you-go on.
    EnablePayg,
}

impl CreditLimitCardAction {
    fn label(self) -> &'static str {
        match self {
            Self::PurchaseCredits => "Buy more credits",
            Self::IncreasePaygLimit => "Increase limit",
            Self::EnablePayg => "Pay as you go",
        }
    }
}

/// Heading, action, and billing URL shown when a max-tier session hits a credit limit.
#[derive(Debug, Clone)]
pub struct CreditLimitBlock {
    pub heading: String,
    pub action: CreditLimitCardAction,
    pub url: String,
}

impl CreditLimitBlock {
    pub fn new(
        heading: impl Into<String>,
        action: CreditLimitCardAction,
        url: impl Into<String>,
    ) -> Self {
        Self {
            heading: heading.into(),
            action,
            url: url.into(),
        }
    }
}

impl BlockContent for CreditLimitBlock {
    fn output(&self, _ctx: &BlockContext) -> BlockOutput {
        let theme = Theme::current();
        let lines = vec![
            BlockLine::styled(Line::from(Span::styled(
                self.heading.clone(),
                Style::default().fg(theme.text_primary),
            ))),
            BlockLine::styled(Line::from(Span::styled(
                self.action.label().to_string(),
                theme.muted(),
            ))),
            BlockLine::styled(Line::from(Span::styled(self.url.clone(), theme.muted()))),
        ];
        BlockOutput { lines }
    }

    fn accent(&self, _ctx: &BlockContext) -> Option<AccentStyle> {
        None
    }

    fn has_vpad_for(&self, _appearance: &AppearanceConfig) -> bool {
        false
    }

    fn is_foldable(&self) -> bool {
        false
    }

    fn is_selectable(&self) -> bool {
        true
    }
}
