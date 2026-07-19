# Residual work: Bitcoin-native Routstr + wallet (2026-07-19)

## Done this pass (OR balance fetch gate + TUI dry-run full hex)

| Item | Status |
|------|--------|
| Pure `should_fetch_openrouter_balance` / `_for_model_id` (shell) | **Done** |
| `Effect::FetchBilling { fetch_openrouter }` + product helpers | **Done** |
| App-level `FetchAppBilling` skips OR network (no active model) | **Done** |
| Model switch re-fetches billing so dual-footer appears without waiting for turn end | **Done** |
| Dual-footer still correct when both OR + Grok balances known | **Unchanged** (fetch OR only on active OR model; xAI still always fetched) |
| TUI dry-run spend: full raw hex in shared prepared lines | **Done** (`format_spend_prepared_lines`) |
| Live CDK mint/refund / LDK BOLT11 | **Still residual** (flags remain false; no fake success) |

## Done prior pass (pager settings Bool `routstr_enabled`)

| Item | Status |
|------|--------|
| Settings Bool `routstr_enabled` (Models, SHELL-owned, restart_required) | **Done** |
| `ALL_SETTINGS_EXERCISED` + keyboard Space + mouse value-column tests | **Done** (`settings_e2e`) |
| Persist `[features].routstr_enabled` via specialized merge (no Features splat) | **Done** (`merge_features_settings_writable`) |
| `set_routstr_enabled` + `Effect::PersistSetting` + rollback to default/`None` | **Done** |
| AppView / PagerLocalSnapshot mirrors; event_loop load; settings modal snapshot | **Done** |
| Live CDK mint/refund / LDK BOLT11 | **Still residual** (flags remain false; no fake success) |

## Done prior pass (CDK/LN product seams + honest gates)

| Item | Status |
|------|--------|
| `default_cashu_backend()` / `default_lightning_backend()` product factories | Done (return stubs today) |
| CLI/TUI topup/refund via factories → `topup_next_steps_for_backends` / `refund_next_steps_for_backend` | Done (no hard-coded Stub types only in product entry) |
| TDD capability gates: stub never invents invoice/refund; mock live success → live copy; mock live Failed → honest failure (not "not wired yet") | Done (`funding_cli` tests) |
| Optional empty features `cashu-cdk` / `ldk` (no heavy deps; flags still false) | Done (`Cargo.toml`; not in default CI) |
| Gate Routstr balance fetch on `[features] routstr_enabled` | Done (`should_fetch_routstr_balance` + disk read in `fetch_routstr_balance_msats`) |

## Done prior pass (broadcast + PSBT spend CLI/TUI)

| Item | Status |
|------|--------|
| `TxBroadcaster` trait + `MockTxBroadcaster` (offline tests) | Done (`explorer`) |
| mempool.space-shaped `POST /api/tx` via rate-limited path (`post_no_cache` / no cache) | Done (`MempoolHttpClient::broadcast_raw_tx_hex`, `explorer-http`) |
| Pure helpers: `transaction_to_raw_hex`, `broadcast_outcome_from_http`, `parse_broadcast_txid_body` | Done |
| `PreparedSpend` + `select_and_prepare_bip84_spend` + `broadcast_raw_tx` | Done (`descriptor_wallet`) |
| Never claim broadcast success without broadcaster `Accepted` + parseable txid | Done (unit tests) |
| Live broadcast reject-path test `#[ignore]` + feature-gated | Done |
| CLI: `grok routstr spend <addr> <sats> [--broadcast] [--fee-rate N]` dry-run default | Done |
| TUI: `/routstr spend <addr> <sats> [broadcast] [fee=N]` stages pending; `/routstr unlock` authorizes (no BIP-39 on spend line) | Done |
| SeedVault unlock + re-entry gates; keyring errors never mint; seeds only SeedVault | Done |
| Cashu/LN capability stubs **not** flipped | Unchanged (honest) |

## Done prior pass (PSBT build/sign from CoinSelection)

| Item | Status |
|------|--------|
| Unsigned PSBT from fee-aware `CoinSelection` + payment/change addresses | Done (`build_unsigned_psbt`) |
| Balance checks (inputs = payment + change + fee); valid 64-hex outpoints | Done |
| BIP84 P2WPKH sign via `bitcoin::psbt::Psbt::sign` + master xpriv | Done (`sign_psbt_bip84_p2wpkh`) |
| P2WPKH finalize (`final_script_witness`) + extract | Done |
| Honest `SignOutcome::Partial` when inputs not in derivation gap | Done |
| Seed zeroize after master creation; `BuiltPsbt` Debug omits secrets | Done |

## Done prior pass (live mempool ChainSource + fee-aware select)

| Item | Status |
|------|--------|
| Live `MempoolChainSource` behind `explorer-http` (mempool `/address/{addr}/utxo`) | Done |
| Pure `parse_mempool_address_utxos` (offline unit tests; tip → confirmations) | Done |
| `mempool_api_address_utxo_url` + `MempoolHttpClient::fetch_address_utxos` (rate-gated) | Done |
| Fee-aware coin selection (`select_coins_with_fee` / `select_coins_ex`, P2WPKH vb heuristics) | Done |
| `CoinSelection.fee_sats`; dust change folded into fee; fee shortfall errors | Done |
| Default CI offline (MockChainSource only); live UTXO test `#[ignore]` + feature-gated | Done |

## Done prior pass (descriptor UTXO + capability seams + TUI QR)

| Item | Status |
|------|--------|
| Descriptor BIP84 wallet surface: account xpub descriptors, receive/change gap | Done (`descriptor_wallet`) |
| `ChainSource` + `MockChainSource`; `list_unspent` / `balance` / `select_coins` | Done (mock + optional live mempool) |
| Cashu `CashuBackend` + `CashuCapabilities` (`mint_live`/`spend_live`/`refund_live` = false on stub) | Done |
| Lightning `InvoiceOutcome` + capability flags; stub never invents BOLT11 or pay success | Done |
| `topup_next_steps_for_backends` / `refund_next_steps_for_backend` capability-aware entry points | Done (`funding_cli`) |
| TUI fund complete: BIP21 QR matrix + clipboard copy + toast on task agent | Done (pager `routstr`) |
| `/routstr qr [address]` (or last watch address) re-show + copy | Done |
| topup/refund remain honest stubs until CDK/LN live (no fake invoice/refund) | Done |

## Done prior (TUI fund + watch + honesty + clamp)

| Item | Status |
|------|--------|
| TUI `/routstr` fund path (probe + unlock re-entry; same gate invariants as CLI) | Done |
| Shared pure helpers: `fund_path_decision_from_load`, `returning_user_reveal_after_reentry`, topup/refund copy | Done (`funding_cli`) |
| Pager slash `/routstr` + Actions/Effects/TaskResults (balance, fund, unlock, topup, refund, watch, stop) | Done |
| Background address watch task in pager (`RoutstrWatchLoop` + generation lifecycle) | Done |
| topup/refund honest next-steps (CLI + TUI share copy; no live mint claim) | Done |
| OpenRouter "can only afford N max_tokens" clamp+retry same key before failover | Done (sampler) |
| Mid-turn toast on OpenRouter/Routstr → Grok API provider failover | Done (pager) |
| Welcome-screen Routstr balance line (app-level fetch already populated) | Done |
| Routstr 402 → Grok API failover smoke (resolve + rotate tests) | Done |
| WatchTaskLifecycle + WatchSession multi-poll (injected producer) | Done |
| OR → Grok API **provider** failover (`FailoverProvider`, rotate, rebuild_client) | Done (prior) |
| Attach first-party session / `XAI_API_KEY` when resolving OpenRouter / Routstr | Done (prior) |
| Subagent inherits parent `failover_api_keys` + providers | Done (prior) |
| Footer: on OpenRouter model, show OR credits **and** `Grok used: N%` when both known | Done (prior) |
| OpenRouter credit failure UI (not SuperGrok "weekly limit") | Done (prior) |

## Done earlier (Routstr foundations)

| Item | Status |
|------|--------|
| Wire `MnemonicBackupGate` + `UnlockSession` into funding CLI before ShowAddress | Done (`funding_cli` + `grok routstr fund`) |
| `grok routstr balance` / `topup` / `refund` / `fund` clap + binary wire-up | Done |
| Address watcher: poll → FundingWizard confirmations (injected producer) | Done |
| `MempoolHttpClient` path for watcher (`explorer-http` helper) | Done |
| Fund: keyring error ≠ mint new wallet; store before address print | Done |
| Fund: AEAD password unlock when keyring miss + file present | Done |
| `begin_reentry_without_display` for returning unlock | Done |
| SeedVault `UnlockSession` idle TTL + zeroize on expire/lock | Done |
| `MnemonicBackupGate` show-once + full re-entry | Done |
| `FundingWizard::show_address` gated on backup confirm | Done |
| `ROUTSTR_GROK_45_MODEL` confirmed live as `grok-4.5` | Done |
| Pager credit footer + `/usage` Routstr paths | Done |
| Prior foundations (BIP-39, AEAD vault, NIP-06, Routstr auth, wizards, …) | Still done |

### Settings note

- Pager settings Bool **Routstr** (`routstr_enabled`) is in Models (SHELL-owned, default on, restart_required).
- Writes `[features] routstr_enabled` via specialized merge (never wholesale Features).
- When false: catalog omit **and** Routstr balance network fetch skipped.
- Toast: "… (restart to apply catalog)". Balance gate re-reads disk after save without restart.

### OpenRouter balance fetch note

- Product gate: `should_fetch_openrouter_balance(active_model_is_openrouter)` /
  `should_fetch_openrouter_balance_for_model_id(Option<&str>)`.
- Pager: `Effect::FetchBilling { fetch_openrouter }` derived from the agent's
  current catalog id (`openrouter-*`). App-level `FetchAppBilling` always passes
  `false` (welcome has no active model / dual-footer).
- Model switch success emits a silent `FetchBilling` so switching **to** OR
  refreshes USD credits without waiting for turn end.
- Dual-footer (`Credits left: $N · Grok used: N%`) still requires both balances
  known; OR is fetched only when the active model is OR-backed; xAI billing is
  unchanged.
- `fetch_openrouter_credit_balance_cents` remains ungated (tests / explicit key);
  product wrappers apply the pure helper first.

### Live catalog note (ROUTSTR_GROK_45_MODEL)

- Fetched `GET https://api.routstr.com/v1/models` (2026-07-18).
- Match: `id: "grok-4.5"`, name `xAI: Grok 4.5`,
  `canonical_slug: "x-ai/grok-4.5-20260708"`.
- Constant kept as short OpenAI-compatible `id` (`grok-4.5`), not the slug.
- Offline CI: `#[ignore]` test
  `auth::routstr::attribution_tests::live_routstr_grok_45_model_in_catalog`.

## Residual (next implement)

### P0 / polish
1. Live keyring integration test behind `#[ignore]` + CI secret-service fixture (optional).
2. Optional: emergency mnemonic re-print only if store fails after backup (today: hard error + "do not fund" + keep paper backup).
3. New-wallet TUI still routes to private CLI (`grok routstr fund`) so recovery words never hit chat history. Optional private modal later.
4. Spend path: live UTXO/broadcast require network; dry-run still needs funded wallet UTXOs. Optional offline mock product mode not shipped.

### P1 / product surfaces
1. ~~Optional tighter gate: OpenRouter balance fetch only when active model is OR / dual-footer needed~~ **Done this pass**.
2. Wire `topup` / `refund` to **real** CDK/LN when those stacks land: flip `mint_live` / `refund_live` / `bolt11_*_live` only with tested impls; swap `default_*_backend()` factories.
3. Optional: dedicated QR pane widget (today: Unicode QR matrix in system block + clipboard toast).
4. ~~Optional: print full raw hex in TUI dry-run~~ **Done this pass** (shared prepared lines include full hex; CLI still also writes hex alone on stdout for pipes).

### P2 / spend path + explorers
1. Multi-sig / non-P2WPKH finalize residual (only single-key P2WPKH finalized).
2. Optional full `bdk_wallet` electrum/esplora sync if still needed beyond mempool UTXO ChainSource.
3. Persist WatchSession across pager process restarts (today: in-process generation loop).
4. Optional: RBF / CPFP-aware fee estimation (today: flat sat/vB + P2WPKH vb heuristics; RBF sequence set on built inputs).
5. Electrum push broadcaster alternative (mempool.space POST wired).

### P3 / LDK
1. `ldk-node` (or LDK) from BIP-39 seed; BOLT11 pay + invoice create with live capability flags.
2. Enable optional `ldk` feature with real deps; keep factory returning live impl only when tested.
3. Channel open to Routstr-recommended peer (API discovery).
4. BOLT12 only when peer+stack support; keep `BOLT12_SUPPORTED` honest (`false`).

### P4 / CDK Cashu
1. CDK mint/wallet for `cashuA` acquire/spend against Routstr (`CashuBackend` live impl).
2. Enable optional `cashu-cdk` feature with real deps; flip `mint_live`/`spend_live`/`refund_live` only when green.
3. Prefer spend Cashu over large hot `sk-` float; refund path (`refund_live`).

### P5 / docs & packaging
1. Shell README Routstr section — **done** (login, toggle, fund pointer).
2. Nix/CI: ensure `grok-bitcoin-wallet` stays in workspace checks; optional `explorer-http` job not required for default CI; do **not** enable `cashu-cdk`/`ldk` in default CI until deps land.
3. Language grep gate already in `scripts/bitcoin-routstr-validate.sh`.

## Next `/implement` prompt (copy)

```text
Continue Bitcoin-native Routstr from RESIDUAL.md (CDK/LN live backends).

OR balance fetch gate + TUI dry-run full hex landed. Do not regress:
  cargo test -p grok-bitcoin-wallet --lib
  cargo test -p xai-grok-shell --lib openrouter_resolve
  cargo test -p xai-grok-shell --lib should_fetch_openrouter
  cargo test -p xai-grok-sampler --lib rotate_failover
  cargo test -p xai-grok-pager --lib credit_bar
  cargo test -p xai-grok-pager --lib routstr
  cargo test -p xai-grok-pager --lib show_usage_fetches_openrouter
  cargo test -p xai-grok-pager --test settings_e2e every_registered_setting_is_exercised
  cargo test -p xai-grok-pager --test settings_e2e routstr_enabled

1. Wire topup/refund to real CDK/LN when stacks land; flip capability flags only when live; keep stubs honest.
2. Optional polish: multi-sig/non-P2WPKH finalize honesty; WatchSession persistence across pager restarts.
3. Do not claim BOLT12; do not store BIP-39 in CredentialsStore.
4. cargo test -p grok-bitcoin-wallet --lib
   cargo test -p xai-grok-shell --lib routstr
   cargo test -p xai-grok-shell --lib openrouter_resolve
   cargo test -p xai-grok-sampler --lib rotate_failover
   cargo test -p xai-grok-pager --lib credit_bar
   cargo test -p xai-grok-pager --lib routstr
   ./scripts/bitcoin-routstr-validate.sh
```

## Test commands (this pass)

```bash
cargo fmt --all
cargo test -p grok-bitcoin-wallet --lib
cargo test -p xai-grok-shell --lib routstr
cargo test -p xai-grok-shell --lib openrouter
cargo test -p xai-grok-shell --lib should_fetch_openrouter
cargo test -p xai-grok-shell --lib openrouter_resolve
cargo test -p xai-grok-shell --lib util::config::persist::tests::features_routstr_enabled_merge_preserves_siblings_and_skips_none
cargo test -p xai-grok-sampler --lib rotate_failover
cargo test -p xai-grok-pager --lib credit_bar
cargo test -p xai-grok-pager --lib routstr
cargo test -p xai-grok-pager --lib show_usage_fetches_openrouter
cargo test -p xai-grok-pager --lib agent_needs_openrouter
cargo test -p xai-grok-pager --lib set_routstr_enabled
cargo test -p xai-grok-pager --lib switch_model_complete_success
cargo test -p xai-grok-pager --test settings_e2e routstr_enabled
cargo test -p xai-grok-pager --test settings_e2e every_registered_setting_is_exercised
cargo clippy -p grok-bitcoin-wallet --lib -- -D warnings
cargo clippy -p xai-grok-shell --lib -- -D warnings
cargo clippy -p xai-grok-pager --lib -- -D warnings
./scripts/bitcoin-routstr-validate.sh
```

## Validation ran (2026-07-19 residual implement — OR fetch gate + TUI full hex)

| Check | Result |
|-------|--------|
| `cargo fmt --all` | pass |
| `cargo test -p grok-bitcoin-wallet --lib` | pass (173) |
| `cargo test -p xai-grok-shell --lib routstr` | pass (26 + 1 ignored) |
| `cargo test -p xai-grok-shell --lib openrouter` | pass (19; includes `should_fetch_openrouter_balance_only_for_active_or_model`) |
| `cargo test -p xai-grok-shell --lib openrouter_resolve` | pass (2) |
| `cargo test -p xai-grok-sampler --lib rotate_failover` | pass (3) |
| `cargo test -p xai-grok-pager --lib credit_bar` | pass (41; dual-footer tests unchanged) |
| `cargo test -p xai-grok-pager --lib routstr` | pass (28) |
| `cargo test -p xai-grok-pager --lib show_usage_fetches_openrouter` | pass |
| `cargo test -p xai-grok-pager --lib agent_needs_openrouter` | pass |
| `cargo test -p xai-grok-pager --lib set_routstr_enabled` | pass |
| `cargo test -p xai-grok-pager --lib switch_model_complete_success` | pass |
| `cargo test -p xai-grok-pager --lib switch_model_complete_persists` | pass |
| `cargo test -p xai-grok-pager --lib switch_to_non_reasoning` | pass |
| `cargo test -p xai-grok-pager --test settings_e2e routstr_enabled` | pass (2) |
| `cargo test -p xai-grok-pager --test settings_e2e every_registered_setting_is_exercised` | pass |
| `cargo clippy -p grok-bitcoin-wallet --lib -- -D warnings` | pass |
| `cargo clippy -p xai-grok-shell --lib -- -D warnings` | pass |
| `cargo clippy -p xai-grok-pager --lib -- -D warnings` | pass |
| `./scripts/bitcoin-routstr-validate.sh` | pass |
| Cashu/LN live flags | still false (honest) |
| OpenRouter balance fetch | gated on active OR model |
| TUI dry-run raw hex | full hex in prepared lines |
| Network broadcast of signed tx | wired prior (dry-run default; `--broadcast` / TUI `broadcast` explicit) |
