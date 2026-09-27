# The status chip shows weekly limits

One grok-oss session calls the billing APIs. The status chip shows this week's included SuperGrok period usage. SuperGrok is a paid product. Included SuperGrok period limits are not SuperGrok dollar credits, and they are not console API credits.

When 28% of that week is used, the chip reads `limits 28%`. Hover reads `72% left`. Click opens Credits and Limits. The header does not say `SuperGrok period`, a workspace name, `behind linear burn`, `15m`, or `24h`.

The Limits tab shows ahead of a linear week, or behind. That pacing stays on the Limits tab.

The Credits tab fields are `Personal credits` and `Console API credits`. Console API credits are the business credits. Personal credits are the personal account's SuperGrok dollar credits. They are shown. The automatic path does not spend them. A failed fetch says `not available`. A figure from chat is not a balance.

`Use credits` changes the next request to console API credits while included limits remain and that balance is available. A real SuperGrok HTTP 402 fails over to console API credits when they are available. A client printout of 100% does not. When included limits and console API credits are both out, the chip shows `2d 4h 12m`.

One session calls the billing APIs at most once a minute, including a card open and `/limits` refresh. Other sessions ask that session on `$GROK_HOME/limits_billing_ask.sock`. That socket is not `$GROK_HOME/leader.sock`. This is not a new daemon. If the first session exits, exactly one successor calls, and the others ask that successor.

Tests:

- `status_row_paints_weekly_limits_used_and_hover_shows_percent_remaining`
- `clicking_the_chip_opens_the_card_and_the_limits_tab_shows_ahead_or_behind_a_linear_week`
- `credits_tab_shows_personal_credits_separate_from_console_api_credits_and_a_failed_fetch_is_not_a_balance`
- `use_credits_spends_console_api_credits_while_limits_remain_and_does_not_spend_personal`
- `real_402_uses_console_api_credits_when_available_and_a_100_percent_printout_does_not`
- `both_limits_and_console_api_credits_exhausted_shows_days_hours_minutes_until_reset`
- `second_session_asks_the_first_over_ipc_and_does_not_call_the_api`
- `forced_refresh_inside_one_minute_does_not_call_the_api_again`
- `when_the_first_session_exits_exactly_one_successor_calls_the_api`

User guide: `crates/codegen/xai-grok-pager/docs/user-guide/25-limits.md`.
