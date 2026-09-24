# Header shows the token count once

Source: screenshot Thu Sep 24, 2026, 12:10 PM, grok-build session.

The status bar shows `↓384.7k 384K / 500K`. Those are the same count. Drop the verbose `↓384.7k`. Keep `384K / 500K`.

The paint is `crates/codegen/xai-grok-pager/src/views/context_bar.rs`. The normal form is `8.5K / 1.0M`. The named test expects `207K / 500K` with no leading `↓207k`.

Status: the verbose down-arrow prefix is gone from the context bar. The bar keeps one count, `207K / 500K` when the windows match. `views::context_bar::tests::context_chip_keeps_unlabeled_used_over_total_when_windows_match` passed.
