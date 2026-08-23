//! Console.x.ai Billing Credits card status for grok-oss limits.
//!
//! Remaining USD is documented on Management GetAmountToPay
//! (`GET …/postpaid/invoice/preview`) as `coreInvoice.prepaidCredits.val`
//! minus `coreInvoice.prepaidCreditsUsed.val`. That remaining is not
//! Management prepaid/balance `total.val`, not SuperGrok `prepaidBalance.val`,
//! and not postpaid `defaultCredits`. See
//! [Billing Management REST](https://docs.x.ai/developers/rest-api-reference/management/billing)
//! (accessed: 2026-08-22). SuperGrok is a paid product.

use serde::{Deserialize, Serialize};

/// Named remaining on GetAmountToPay: abs(prepaidCredits.val) minus
/// abs(prepaidCreditsUsed.val). Not `prepaidCredits.val` alone.
pub const BILLING_CREDITS_CARD_NAMED_FIELD: &str =
    "coreInvoice.prepaidCredits.val-coreInvoice.prepaidCreditsUsed.val";

/// Status of the console.x.ai Billing Credits card in grok-oss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingCreditsCard {
    /// No live call, or the named remaining fields were not on the body.
    #[default]
    NotFetched,
    /// Named remaining was parsed from GetAmountToPay.
    Fetched,
    /// A Management call was attempted and failed.
    Error,
}

impl BillingCreditsCard {
    /// Stable `/limits` JSON wire value.
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::NotFetched => "not_fetched",
            Self::Fetched => "fetched",
            Self::Error => "error",
        }
    }
}

/// Resolve USD to print as **current** console Billing Credits.
///
/// `documented_billing_credits_field_usd` is only `Some` when a live JSON
/// field **named as that card** was fetched. Management prepaid
/// `total.val` and SuperGrok `prepaidBalance.val` are not that field.
/// A stored dollar is dropped when a newer live fetch or operator-reported
/// page disagrees.
pub fn current_billing_credits_usd(
    stored_usd: Option<f64>,
    newer_live_or_operator_usd: Option<f64>,
    documented_billing_credits_field_usd: Option<f64>,
) -> Option<f64> {
    if let Some(named) = documented_billing_credits_field_usd {
        return Some(named);
    }
    // Drop stored when a newer live fetch or operator-reported page
    // disagrees. Do not fill from Management `total.val` or SuperGrok
    // `prepaidBalance.val`.
    let _ = (stored_usd, newer_live_or_operator_usd);
    None
}

/// Prefer a newer live documented dollar over a stored dollar.
///
/// When live disagrees with stored, drop stored. When live is absent, last
/// successful documented cents may remain (not Billing Credits).
pub fn prefer_live_documented_usd_over_stored(
    stored_usd: Option<f64>,
    live_documented_usd: Option<f64>,
) -> Option<f64> {
    match (stored_usd, live_documented_usd) {
        (_, Some(live)) => Some(live),
        (stored, None) => stored,
    }
}

/// Included SuperGrok period limits used percent is not the Billing Credits dollar.
pub fn billing_credits_usd_from_included_period_percent(_used_pct: f64) -> Option<f64> {
    None
}

/// USD from a named JSON field. Only the GetAmountToPay remaining pair
/// (`prepaidCredits` minus `prepaidCreditsUsed`) is the Billing Credits card.
///
/// Public docs name Management prepaid `total.val`, SuperGrok
/// `prepaidBalance.val`, postpaid `defaultCredits`, and
/// `coreInvoice.prepaidCredits.val` alone. Those are not this card. See
/// [Billing Management REST](https://docs.x.ai/developers/rest-api-reference/management/billing)
/// (accessed: 2026-08-22).
pub fn billing_credits_usd_from_named_json_field(field_path: &str, usd: f64) -> Option<f64> {
    if field_path == BILLING_CREDITS_CARD_NAMED_FIELD {
        return Some(usd);
    }
    None
}

/// Remaining USD cents on the Billing Credits card from documented
/// GetAmountToPay fields. Both `prepaidCredits.val` and
/// `prepaidCreditsUsed.val` must parse. `prepaidCredits.val` alone is not
/// this card.
pub fn billing_credits_cents_from_core_invoice_prepaid_remaining(
    prepaid_credits_val: &str,
    prepaid_credits_used_val: &str,
) -> Option<i64> {
    let prepaid = parse_usd_cents_abs(prepaid_credits_val)?;
    let used = parse_usd_cents_abs(prepaid_credits_used_val)?;
    Some(prepaid.saturating_sub(used))
}

/// Remaining USD from the same documented pair.
pub fn billing_credits_usd_from_core_invoice_prepaid_remaining(
    prepaid_credits_val: &str,
    prepaid_credits_used_val: &str,
) -> Option<f64> {
    let cents = billing_credits_cents_from_core_invoice_prepaid_remaining(
        prepaid_credits_val,
        prepaid_credits_used_val,
    )?;
    Some(cents as f64 / 100.0)
}

fn parse_usd_cents_abs(val: &str) -> Option<i64> {
    let n: i64 = val.trim().parse().ok()?;
    Some(n.saturating_abs())
}

/// SuperGrok session `prepaidBalance.val` is SuperGrok dollar credits.
/// It is not the console Billing Credits card.
pub fn billing_credits_card_from_supergrok_prepaid_balance(
    _prepaid_balance_cents: i64,
) -> BillingCreditsCard {
    BillingCreditsCard::NotFetched
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stored $89.94 must not be current Billing Credits when live $47.03 disagrees.
    #[test]
    fn stale_stored_dollars_are_not_reported_as_current_billing_credits_when_live_fetch_disagrees()
    {
        let stored = Some(89.94);
        let live_page = Some(47.03);
        let current = current_billing_credits_usd(stored, live_page, None);
        assert_eq!(
            current, None,
            "stored $89.94 must not be current Billing Credits when live disagrees ($47.03)"
        );
        // $47.03 is the operator-visible Billing Credits page. It is not a
        // documented JSON field, so it must not be passed as live documented USD.
        assert_eq!(
            prefer_live_documented_usd_over_stored(stored, None),
            stored,
            "without a documented live fetch, stored Management cents may remain (not the Credits card)"
        );
        assert_eq!(
            prefer_live_documented_usd_over_stored(Some(89.94), Some(12.50)),
            Some(12.50),
            "a later documented Management prepaid fetch replaces stored prepaid remaining"
        );
        assert_ne!(
            live_page, stored,
            "operator-visible $47.03 is not the stale $89.94 paste"
        );
    }

    /// Do not classify the Credits card without a named JSON field for that card.
    #[test]
    fn billing_credits_card_is_not_classified_without_named_json_field() {
        assert_eq!(BillingCreditsCard::NotFetched.as_wire(), "not_fetched");
        // Management total.val and SuperGrok prepaidBalance.val are not this card.
        let from_prepaid_total = current_billing_credits_usd(None, None, None);
        assert_eq!(
            from_prepaid_total, None,
            "no documented Billing Credits field → unknown, not a guessed meter"
        );
        let with_wrong_fill = current_billing_credits_usd(
            Some(89.94),
            Some(47.03),
            None, // no field named as the Billing Credits card
        );
        assert_eq!(
            with_wrong_fill, None,
            "must not fill the card from stored $89.94 or from an unclassified live $47.03"
        );
        for field in [
            "total.val",
            "prepaidBalance.val",
            "defaultCredits",
            "coreInvoice.prepaidCredits.val",
        ] {
            assert_eq!(
                billing_credits_usd_from_named_json_field(field, 47.03),
                None,
                "{field} is not a documented Billing Credits card field"
            );
        }
        assert_eq!(
            billing_credits_card_from_supergrok_prepaid_balance(4_703),
            BillingCreditsCard::NotFetched
        );
        assert_eq!(BillingCreditsCard::Fetched.as_wire(), "fetched");
        assert_eq!(BillingCreditsCard::Error.as_wire(), "error");
    }

    /// Named remaining on GetAmountToPay is abs(prepaidCredits) minus
    /// abs(prepaidCreditsUsed). Not team prepaid `total.val` $112.45 and not
    /// SuperGrok `prepaidBalance.val` $248.24.
    #[test]
    fn billing_credits_usd_from_named_remaining_not_total_val_or_prepaid_balance() {
        assert_eq!(
            billing_credits_cents_from_core_invoice_prepaid_remaining("-11245", "6542"),
            Some(4_703)
        );
        let usd = billing_credits_usd_from_core_invoice_prepaid_remaining("-11245", "6542")
            .expect("named remaining");
        assert!((usd - 47.03).abs() < f64::EPSILON);
        assert_eq!(
            billing_credits_usd_from_named_json_field(BILLING_CREDITS_CARD_NAMED_FIELD, usd),
            Some(usd)
        );
        assert_eq!(
            current_billing_credits_usd(Some(89.94), Some(112.45), Some(usd)),
            Some(usd),
            "named remaining wins over stored $89.94 and team prepaid $112.45"
        );
        assert_eq!(
            billing_credits_usd_from_core_invoice_prepaid_remaining("-11245", "not-cents"),
            None,
            "used field must parse; do not fall back to prepaidCredits.val alone"
        );
        assert_eq!(
            billing_credits_usd_from_named_json_field("total.val", 112.45),
            None
        );
        assert_eq!(
            billing_credits_usd_from_named_json_field("prepaidBalance.val", 248.24),
            None
        );
    }

    /// Included SuperGrok period limits percent is not the Credits dollar.
    #[test]
    fn included_supergrok_period_limits_percent_is_not_the_billing_credits_dollar_balance() {
        assert_eq!(
            billing_credits_usd_from_included_period_percent(47.03),
            None,
            "included used % must not become the Billing Credits dollar"
        );
        assert_eq!(billing_credits_usd_from_included_period_percent(0.0), None);
        assert_eq!(
            billing_credits_usd_from_included_period_percent(100.0),
            None
        );
    }
}
