//! Lightning capability trait and stubs.
//!
//! Full LDK / `ldk-node` integration is residual. This module defines the
//! surface the funding wizard and Routstr top up will call, with honest
//! BOLT12 support flags.
//!
//! Capability flags (`bolt11_pay_live`, `bolt11_invoice_live`, `bolt12_supported`)
//! must stay accurate: stubs never claim a live pay or invoice path.

use crate::BOLT12_SUPPORTED;
use crate::error::{Result, WalletError};

/// BOLT11 pay request (invoice string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bolt11Invoice(pub String);

/// Result of a pay attempt (stub).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayOutcome {
    /// Stub success with preimage hex (tests).
    Success {
        preimage_hex: String,
    },
    /// Not implemented / deferred.
    Unsupported(&'static str),
    Failed(String),
}

/// Result of attempting to create a BOLT11 receive invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvoiceOutcome {
    /// Live invoice string (only when `bolt11_invoice_live` is true).
    Created {
        bolt11: String,
    },
    /// Backend cannot create invoices in this build.
    Unsupported(&'static str),
    Failed(String),
}

/// Static capability snapshot for UI / CLI honesty copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightningCapabilities {
    /// Can pay a BOLT11 invoice end-to-end (LDK path live).
    pub bolt11_pay_live: bool,
    /// Can create a BOLT11 receive invoice (LDK path live).
    pub bolt11_invoice_live: bool,
    /// BOLT12 offers supported (must match [`crate::BOLT12_SUPPORTED`]).
    pub bolt12_supported: bool,
}

/// Default pre-LDK capabilities: nothing live; BOLT12 never claimed.
pub const STUB_LIGHTNING_CAPABILITIES: LightningCapabilities = LightningCapabilities {
    bolt11_pay_live: false,
    bolt11_invoice_live: false,
    bolt12_supported: BOLT12_SUPPORTED,
};

/// Lightning operations the wallet exposes to upper layers.
pub trait LightningCapability {
    /// Capability flags for this backend (must not over-claim).
    fn capabilities(&self) -> LightningCapabilities {
        STUB_LIGHTNING_CAPABILITIES
    }

    /// Pay a BOLT11 invoice (may be stubbed).
    fn pay_bolt11(&self, invoice: &Bolt11Invoice) -> Result<PayOutcome>;

    /// Create a BOLT11 invoice for receiving `amount_sats` (may be stubbed).
    ///
    /// Stubs **must** return [`InvoiceOutcome::Unsupported`] and never a
    /// fabricated `lnbc…` string that looks pay-able.
    fn create_bolt11_invoice(&self, _amount_sats: Option<u64>) -> Result<InvoiceOutcome> {
        Ok(InvoiceOutcome::Unsupported(
            "LDK BOLT11 invoice create not wired (stub LightningCapability)",
        ))
    }

    /// Whether BOLT12 offers are supported in this build/runtime.
    fn bolt12_supported(&self) -> bool {
        self.capabilities().bolt12_supported
    }

    /// Pay a BOLT12 offer. Default rejects when unsupported.
    fn pay_bolt12_offer(&self, _offer: &str) -> Result<PayOutcome> {
        if !self.bolt12_supported() {
            return Err(WalletError::Bolt12Unsupported);
        }
        Ok(PayOutcome::Unsupported("BOLT12 pay not implemented"))
    }
}

/// No-op Lightning backend for unit tests and pre-LDK builds.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubLightning;

impl LightningCapability for StubLightning {
    fn capabilities(&self) -> LightningCapabilities {
        STUB_LIGHTNING_CAPABILITIES
    }

    fn pay_bolt11(&self, invoice: &Bolt11Invoice) -> Result<PayOutcome> {
        if invoice.0.trim().is_empty() {
            return Ok(PayOutcome::Failed("empty invoice".into()));
        }
        // Never Success: stub must not claim a completed payment.
        Ok(PayOutcome::Unsupported(
            "LDK BOLT11 pay not wired (stub LightningCapability)",
        ))
    }

    fn create_bolt11_invoice(&self, _amount_sats: Option<u64>) -> Result<InvoiceOutcome> {
        Ok(InvoiceOutcome::Unsupported(
            "LDK BOLT11 invoice create not wired (stub LightningCapability)",
        ))
    }
}

/// Product default Lightning backend for top up / pay CLI+TUI paths.
///
/// Returns an opaque [`LightningCapability`] so a future live LDK type can
/// replace the body without changing the public signature (keep
/// `impl LightningCapability`, or a private owned enum if needed).
///
/// Today this is always [`StubLightning`] (`bolt11_pay_live` /
/// `bolt11_invoice_live` false; `bolt12_supported` matches
/// [`crate::BOLT12_SUPPORTED`]). The optional Cargo feature `ldk` is a
/// **reservation only** until real deps land (RESIDUAL.md P3); enabling it
/// does not change this factory. Product copy routes through
/// [`crate::funding_cli::topup_next_steps_for_backends`].
pub fn default_lightning_backend() -> impl LightningCapability {
    StubLightning
}

/// Channel-open wizard steps toward a Routstr-recommended peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelWizardStep {
    /// Need on-chain funds confirmed.
    NeedConfirmedFunds,
    /// Peer URI / node id resolved from Routstr.
    PeerResolved,
    /// User confirmed capacity + fees.
    UserConfirmed,
    /// Funding transaction broadcast.
    FundingBroadcast,
    /// Channel active / ready for LN payments.
    ChannelActive,
    /// Failed; funds remain on-chain.
    Failed,
}

/// Minimal state machine for channel-to-Routstr-peer flow (no live LN).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelWizard {
    pub step: ChannelWizardStep,
    pub peer_id: Option<String>,
    pub capacity_sats: Option<u64>,
    pub funding_txid: Option<String>,
    pub last_error: Option<String>,
}

impl Default for ChannelWizard {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelWizard {
    pub fn new() -> Self {
        Self {
            step: ChannelWizardStep::NeedConfirmedFunds,
            peer_id: None,
            capacity_sats: None,
            funding_txid: None,
            last_error: None,
        }
    }

    pub fn resolve_peer(&mut self, peer_id: impl Into<String>) -> Result<()> {
        self.ensure(ChannelWizardStep::NeedConfirmedFunds)?;
        self.peer_id = Some(peer_id.into());
        self.step = ChannelWizardStep::PeerResolved;
        Ok(())
    }

    pub fn confirm(&mut self, capacity_sats: u64) -> Result<()> {
        self.ensure(ChannelWizardStep::PeerResolved)?;
        if capacity_sats == 0 {
            return Err(WalletError::Onchain("capacity must be > 0".into()));
        }
        self.capacity_sats = Some(capacity_sats);
        self.step = ChannelWizardStep::UserConfirmed;
        Ok(())
    }

    pub fn mark_funding_broadcast(&mut self, txid: impl Into<String>) -> Result<()> {
        self.ensure(ChannelWizardStep::UserConfirmed)?;
        self.funding_txid = Some(txid.into());
        self.step = ChannelWizardStep::FundingBroadcast;
        Ok(())
    }

    pub fn mark_active(&mut self) -> Result<()> {
        self.ensure(ChannelWizardStep::FundingBroadcast)?;
        self.step = ChannelWizardStep::ChannelActive;
        Ok(())
    }

    pub fn fail(&mut self, err: impl Into<String>) {
        self.last_error = Some(err.into());
        self.step = ChannelWizardStep::Failed;
    }

    fn ensure(&self, expected: ChannelWizardStep) -> Result<()> {
        if self.step != expected {
            return Err(WalletError::ChannelWizard(format!(
                "expected {expected:?}, at {:?}",
                self.step
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolt12_flag_false() {
        let ln = StubLightning;
        assert!(!ln.bolt12_supported());
        assert!(!ln.capabilities().bolt12_supported);
        assert!(matches!(
            ln.pay_bolt12_offer("lno1…"),
            Err(WalletError::Bolt12Unsupported)
        ));
    }

    #[test]
    fn stub_bolt11_unsupported() {
        let ln = StubLightning;
        let out = ln.pay_bolt11(&Bolt11Invoice("lnbc1…".into())).unwrap();
        assert!(matches!(out, PayOutcome::Unsupported(_)));
    }

    #[test]
    fn stub_never_claims_live_invoice_or_pay() {
        let ln = StubLightning;
        let caps = ln.capabilities();
        assert!(!caps.bolt11_pay_live);
        assert!(!caps.bolt11_invoice_live);
        assert!(!caps.bolt12_supported);

        let inv = ln.create_bolt11_invoice(Some(21_000)).unwrap();
        assert!(
            matches!(inv, InvoiceOutcome::Unsupported(_)),
            "stub must not invent a BOLT11: {inv:?}"
        );
        if let InvoiceOutcome::Created { bolt11 } = inv {
            panic!("stub fabricated invoice: {bolt11}");
        }
        let pay = ln
            .pay_bolt11(&Bolt11Invoice("lnbc1reallooking".into()))
            .unwrap();
        assert!(
            !matches!(pay, PayOutcome::Success { .. }),
            "stub must not claim payment success: {pay:?}"
        );
    }

    #[test]
    fn default_lightning_backend_is_stub_with_live_flags_false() {
        let ln = default_lightning_backend();
        let caps = ln.capabilities();
        assert!(!caps.bolt11_pay_live);
        assert!(!caps.bolt11_invoice_live);
        assert!(!caps.bolt12_supported);
        assert_eq!(caps.bolt12_supported, crate::BOLT12_SUPPORTED);
        assert!(matches!(
            ln.create_bolt11_invoice(Some(1)).unwrap(),
            InvoiceOutcome::Unsupported(_)
        ));
        assert!(matches!(
            ln.pay_bolt11(&Bolt11Invoice("lnbc1…".into())).unwrap(),
            PayOutcome::Unsupported(_)
        ));
    }

    #[test]
    fn channel_wizard_happy_path() {
        let mut w = ChannelWizard::new();
        w.resolve_peer("02abc").unwrap();
        w.confirm(100_000).unwrap();
        w.mark_funding_broadcast("txid").unwrap();
        w.mark_active().unwrap();
        assert_eq!(w.step, ChannelWizardStep::ChannelActive);
    }

    #[test]
    fn channel_wizard_rejects_skip() {
        let mut w = ChannelWizard::new();
        assert!(w.confirm(1).is_err());
    }
}
