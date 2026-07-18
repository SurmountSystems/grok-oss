<<<<<<< HEAD
//! The is-managed claim in the gate decision and the staleness refetch: removing the policy sidecar must not downgrade a claimed principal.
=======
//! The is-managed claim in the gate decision and the staleness refetch
//! (sidecar-removal downgrade closure).
>>>>>>> e3fdf3ed (Merge 2 (#4))

use super::super::*;
use super::team;

<<<<<<< HEAD
/// A stripped policy sidecar plus an imposing claim refuses even over a fully forged permissive marker.
/// Without the claim that same state is the documented marker downgrade.
=======
/// Headline: a stripped policy sidecar + imposing claim refuses even over a fully
/// forged permissive marker; without the claim that state is the pre-fix downgrade.
>>>>>>> e3fdf3ed (Merge 2 (#4))
#[test]
fn claim_refuses_stripped_sidecar_even_with_forged_marker() {
    use crate::signed_policy::SignedVerdict;
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    // The forged-marker shape: permissive, nothing served, matching principal.
    let forged = ManagedConfigCache {
        principal: Some("team-007".into()),
        fail_closed: false,
        ..Default::default()
    };
<<<<<<< HEAD

    assert_eq!(
        Some(ManagedPolicyCompromise::SignatureMissing),
=======
    assert!(
>>>>>>> e3fdf3ed (Merge 2 (#4))
        managed_policy_compromised_decision(
            SignedVerdict::NoAuthenticSidecar,
            || true,
            false,
            Some(&forged),
            home,
            &team("team-007")
        ),
        "an imposing claim outranks the forged marker when the policy sidecar is gone"
    );
<<<<<<< HEAD
    assert_eq!(
        None,
        managed_policy_compromised_decision(
=======
    assert!(
        !managed_policy_compromised_decision(
>>>>>>> e3fdf3ed (Merge 2 (#4))
            SignedVerdict::NoAuthenticSidecar,
            || false,
            false,
            Some(&forged),
            home,
            &team("team-007")
        ),
        "without the claim this exact state is the (documented) marker downgrade"
    );
}

<<<<<<< HEAD
/// A policy-sidecar read blip stays lenient: the claim is not consulted on `SidecarUnreadable` (rationale on the variant doc).
=======
/// A policy-sidecar read blip stays lenient: the claim is not consulted on
/// `SidecarUnreadable` (rationale on the variant doc).
>>>>>>> e3fdf3ed (Merge 2 (#4))
#[test]
fn claim_not_consulted_on_sidecar_read_blip() {
    use crate::signed_policy::SignedVerdict;
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    std::fs::write(home.join("requirements.toml"), "[features]\n").unwrap();
    let served = ManagedConfigCache {
        principal: Some("team-007".into()),
        had_requirements: true,
        fail_closed: true,
        ..Default::default()
    };
<<<<<<< HEAD

    assert_eq!(
        None,
        managed_policy_compromised_decision(
=======
    assert!(
        !managed_policy_compromised_decision(
>>>>>>> e3fdf3ed (Merge 2 (#4))
            SignedVerdict::SidecarUnreadable,
            || true,
            false,
            Some(&served),
            home,
            &team("team-007")
        ),
        "a transient sidecar read blip must not refuse, claim or no claim"
    );
}

<<<<<<< HEAD
/// Keyed build: a garbage claim alone (no fail-closed) does not trip the gate or force a refetch.
#[test]
fn garbage_claim_without_fail_closed_is_not_imposing() {
    assert!(crate::signed_policy::verification_active());
=======
/// Dark build: a claim file on disk changes neither the gate nor staleness.
#[test]
fn claim_paths_are_inert_in_dark_build() {
    assert!(!crate::signed_policy::verification_active());
>>>>>>> e3fdf3ed (Merge 2 (#4))
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    mark_managed_config_synced_at(
        home,
        SyncMarker {
            principal: Some("team-a"),
            had_managed_config: false,
            had_requirements: false,
            key_fingerprint: None,
            fail_closed: false,
        },
    );
    std::fs::write(
        home.join(crate::signed_policy::MANAGED_IDENTITY_SIDECAR_FILE),
        "{\"signed_payload\":\"{}\",\"signature\":\"\",\"key_id\":\"\"}",
    )
    .unwrap();
<<<<<<< HEAD

    assert_eq!(
        None,
        managed_policy_compromised_for_at(home, &team("team-a")),
        "garbage claim without fail-closed must not make the gate fail closed"
    );
    assert!(
        !is_managed_config_hard_stale_for_at(home, &team("team-a")),
        "garbage claim without fail-closed must not force a refetch"
    );
}

/// Keyless: a claim file does not affect the gate or staleness.
#[test]
fn claim_paths_are_inert_in_dark_build() {
    crate::signed_policy::test_seam::with_dark(|| {
        assert!(!crate::signed_policy::verification_active());
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        mark_managed_config_synced_at(
            home,
            SyncMarker {
                principal: Some("team-a"),
                had_managed_config: false,
                had_requirements: false,
                key_fingerprint: None,
                fail_closed: false,
            },
        );
        std::fs::write(
            home.join(crate::signed_policy::MANAGED_IDENTITY_SIDECAR_FILE),
            r#"{"signed_payload":"{}","signature":"","key_id":""}"#,
        )
        .unwrap();

        assert_eq!(
            None,
            managed_policy_compromised_for_at(home, &team("team-a")),
            "dark build: a claim file must not make the gate fail closed"
        );
        assert!(
            !is_managed_config_hard_stale_for_at(home, &team("team-a")),
            "dark build: a claim file must not force a refetch"
        );
    });
}
=======
    assert!(
        !managed_policy_compromised_for_at(home, &team("team-a")),
        "dark build: a claim file must not make the gate fail closed"
    );
    assert!(
        !is_managed_config_hard_stale_for_at(home, &team("team-a")),
        "dark build: a claim file must not force a refetch"
    );
}
>>>>>>> e3fdf3ed (Merge 2 (#4))
