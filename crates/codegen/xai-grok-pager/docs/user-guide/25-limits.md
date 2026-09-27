# Limits

SuperGrok is a paid product. This page is the weekly limits chip on the status row, and the Credits and Limits card that click opens. It is not external OpenTelemetry. It is not the Token Economy ledger (`/spend`).

Included SuperGrok period limits are the subscription-included quota for the current SuperGrok billing period. They are not SuperGrok dollar credits. They are not console API credits.

## Status chip

When 28% of this week's included SuperGrok period limits are used, the status chip reads `limits 28%`. Hover reads `72% left`. The chip has a hit rectangle.

The header, the chip, and the hover do not say `SuperGrok period`, a workspace name, `behind linear burn`, `15m`, or `24h`.

Click the chip to open Credits and Limits.

When included limits and console API credits are both out, the chip shows `2d 4h 12m` until that included period resets. If the period end is missing, the reset time says not available. grok-oss does not invent a clock.

## Credits and Limits

The card has two tabs: Credits and Limits. Changing tabs does not change which meter the next request uses.

The Limits tab shows how far ahead of a linear week the included usage is, or how far behind. That pacing stays on the Limits tab. It is not on the header.

The Credits tab has two fields: `Personal credits` and `Console API credits`. The amounts are separate. Console API credits are the business credits. That is the console team prepaid balance. There is not a third field named Business credits.

Personal credits are the personal account's SuperGrok dollar credits. The card shows them. The automatic path does not spend them.

A failed fetch says `not available`. It does not show a dollar amount, and it does not show a leftover number as if that number were current. A figure from chat is not a balance.

## Use credits

`Use credits` changes the next request. While included limits remain and console API credits are available, that next request uses console API credits. It does not spend personal credits.

`Use limits` sends the next request on included limits again.

A real SuperGrok HTTP 402 fails over. After that 402, when console API credits are available, the next request uses console API credits. A client printout of 100% does not change the next request, and it does not fail over.

## One session calls the billing APIs

One grok-oss session calls the billing APIs. It calls them at most once a minute. That cap covers a card open and `/limits` refresh. An ask inside that minute returns the last details and does not call again. Idle sessions do not poll every minute just because the cap allows a call.

Other sessions ask that session. They do not call the billing APIs themselves. The ask uses `$GROK_HOME/limits_billing_ask.sock`. That socket is not the ACP socket at `$GROK_HOME/leader.sock`. This is not a new daemon.

If that session exits, the lock appoints exactly one successor. The other sessions ask the successor. Two sessions do not both become the caller. A dead socket file is not a live leader.

## Tests

These tests are the contract:

- `status_row_paints_weekly_limits_used_and_hover_shows_percent_remaining`
- `clicking_the_chip_opens_the_card_and_the_limits_tab_shows_ahead_or_behind_a_linear_week`
- `credits_tab_shows_personal_credits_separate_from_console_api_credits_and_a_failed_fetch_is_not_a_balance`
- `use_credits_spends_console_api_credits_while_limits_remain_and_does_not_spend_personal`
- `real_402_uses_console_api_credits_when_available_and_a_100_percent_printout_does_not`
- `both_limits_and_console_api_credits_exhausted_shows_days_hours_minutes_until_reset`
- `second_session_asks_the_first_over_ipc_and_does_not_call_the_api`
- `forced_refresh_inside_one_minute_does_not_call_the_api_again`
- `when_the_first_session_exits_exactly_one_successor_calls_the_api`

Named `/limits` words stay on [Authentication](02-authentication.md) and [Slash Commands](04-slash-commands.md#limits). grok-oss limits JSON is a client printout, not xAI billing truth.
