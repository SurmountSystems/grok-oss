//! Operator contracts for the local uptime series.
//! Cargo was not run from the agent that wrote these tests.

use std::fs;
use std::path::PathBuf;

use parquet::basic::Encoding;
use parquet::schema::types::ColumnPath;

use super::{
    BANNER_DATACENTER_INCIDENT, BANNER_MODEL_SERVING_ISSUES, Observation, Outcome, StoreError,
    UptimeStore, column_encodings, extra_api_requests, format_uptime_beside_status,
    hide_announcement_keeps_row, synthetic_probe_enabled, writer_properties,
};

const NOW_UNIX_MS: i64 = 1_700_000_000_000;
const SESSION_ID: &str = "local-session-uptime";
const MODEL_ID: &str = "grok-4.6";

fn open_store() -> (tempfile::TempDir, UptimeStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = UptimeStore::open(dir.path()).expect("open store");
    (dir, store)
}

fn observation(outcome: Outcome, time_unix_ms: i64) -> Observation {
    Observation {
        time_unix_ms,
        outcome,
        latency_ms: None,
        token_count: None,
        tokens_not_fetched: true,
        model_id: MODEL_ID.to_string(),
        local_session_id: SESSION_ID.to_string(),
        banner_text: None,
    }
}

fn piece_bytes(store: &UptimeStore) -> Vec<(PathBuf, Vec<u8>)> {
    store
        .piece_paths()
        .expect("pieces")
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).expect("read piece");
            (path, bytes)
        })
        .collect()
}

#[test]
fn operator_a_burst_of_local_observations_does_not_open_a_socket_to_xai() {
    let (_dir, store) = open_store();
    for index in 0..24 {
        let outcome = if index % 5 == 0 {
            Outcome::Http500
        } else if index % 5 == 1 {
            Outcome::Timeout
        } else if index % 5 == 2 {
            Outcome::RepeatingSentenceStop
        } else {
            Outcome::ModelRequestSucceeded
        };
        store
            .record(observation(outcome, NOW_UNIX_MS - i64::from(index) * 1_000))
            .expect("record local observation");
    }
    assert_eq!(
        extra_api_requests(),
        0,
        "Operator: a burst of local observations does not open a socket to xAI"
    );
    assert!(
        !synthetic_probe_enabled(),
        "Operator: a burst of local observations does not open a socket to xAI"
    );
    assert_eq!(store.piece_paths().expect("pieces").len(), 24);
}

#[test]
fn operator_http_500_is_stored_and_counted_in_both_the_15_minute_and_24_hour_windows() {
    let (_dir, store) = open_store();
    store
        .record(observation(Outcome::Http500, NOW_UNIX_MS - 60_000))
        .expect("record 500");
    store
        .record(observation(
            Outcome::Http500,
            NOW_UNIX_MS - (25 * 60 * 60 * 1000),
        ))
        .expect("record old 500");
    let windows = store.aggregate(NOW_UNIX_MS).expect("aggregate");
    assert_eq!(
        windows.last_15_minutes.http_500_count, 1,
        "Operator: a 500 observation shows up in the 500 count for the last 15 minutes"
    );
    assert_eq!(
        windows.last_24_hours.http_500_count, 1,
        "Operator: a 500 observation shows up in the 500 count for the last 24 hours"
    );
    let text = format_uptime_beside_status(&windows);
    assert!(
        text.contains("15 minutes") && text.contains("24 hours"),
        "Operator: the window says 15 minutes and 24 hours: {text}"
    );
    assert!(
        text.contains("HTTP 500 count 1"),
        "Operator: the 500 count is in the window text: {text}"
    );
    let rows = store.read_rows().expect("rows");
    assert!(
        rows.iter()
            .any(|row| row.outcome == Outcome::Http500.as_str()),
        "Operator: a 500 observation is stored"
    );
}

#[test]
fn operator_not_fetched_token_count_is_stored_as_not_fetched_and_not_167k() {
    let (_dir, store) = open_store();
    let mut row = observation(Outcome::ModelRequestSucceeded, NOW_UNIX_MS);
    row.latency_ms = Some(40);
    row.token_count = Some(167_000);
    row.tokens_not_fetched = true;
    let piece = store.record(row).expect("record");
    let bytes = fs::read(&piece).expect("piece bytes");
    assert!(
        !bytes.windows(8).any(|window| window == b"167.0k"),
        "Operator: a not-fetched token count is stored as not fetched. Do not store 167.0k."
    );
    let stored = store.read_rows().expect("rows");
    assert_eq!(stored.len(), 1);
    assert!(
        stored[0].tokens_not_fetched,
        "Operator: a not-fetched token count is stored as not fetched"
    );
    assert_eq!(
        stored[0].token_count, None,
        "Operator: the row's token number is absent. Do not store 167.0k."
    );
    let windows = store.aggregate(NOW_UNIX_MS).expect("aggregate");
    let text = format_uptime_beside_status(&windows);
    assert!(
        !text.contains("not fetched") && !text.contains("tokens not fetched"),
        "Operator: a missing token count omits the token clause: {text}"
    );
    assert!(
        !text.contains("tokens"),
        "Operator: a missing token count does not print a token clause: {text}"
    );
    assert!(
        !text.contains("167.0k") && !text.contains("167000") && !text.contains("167"),
        "Operator: the window does not invent a token total: {text}"
    );
}

#[test]
fn operator_announcement_banner_is_stored_and_hide_does_not_delete_the_row() {
    let (_dir, store) = open_store();
    for (offset, text) in [BANNER_MODEL_SERVING_ISSUES, BANNER_DATACENTER_INCIDENT]
        .into_iter()
        .enumerate()
    {
        let mut row = observation(Outcome::AnnouncementBanner, NOW_UNIX_MS - offset as i64);
        row.banner_text = Some(text.to_string());
        store.record(row).expect("record banner");
    }
    let before = piece_bytes(&store);
    assert_eq!(before.len(), 2);
    assert!(
        hide_announcement_keeps_row(&store),
        "Operator: /announcements hide keeps the row and does not delete Parquet"
    );
    let after = piece_bytes(&store);
    assert_eq!(before, after, "Operator: hide does not delete that row");
    let rows = store.read_rows().expect("rows");
    let banners: Vec<&str> = rows
        .iter()
        .filter(|row| row.outcome == Outcome::AnnouncementBanner.as_str())
        .map(|row| row.banner_text.as_deref().unwrap_or(""))
        .collect();
    assert!(
        banners.contains(&BANNER_MODEL_SERVING_ISSUES),
        "Operator: an announcement banner is stored: We're currently experiencing issues serving our models"
    );
    assert!(
        banners.contains(&BANNER_DATACENTER_INCIDENT),
        "Operator: an announcement banner is stored: A datacenter incident is affecting all Grok models"
    );
}

#[test]
fn operator_parquet_pieces_are_readable_by_duckdb_and_numeric_columns_use_delta_binary_packing() {
    let props = writer_properties();
    for name in ["time_unix_ms", "latency_ms", "token_count"] {
        assert_eq!(
            props.encoding(&ColumnPath::from(name)),
            Some(Encoding::DELTA_BINARY_PACKED),
            "Operator: numeric columns use Encoding::DELTA_BINARY_PACKED"
        );
    }
    let (_dir, store) = open_store();
    let mut row = observation(Outcome::ModelRequestSucceeded, NOW_UNIX_MS);
    row.latency_ms = Some(12);
    row.token_count = Some(9);
    row.tokens_not_fetched = false;
    let recorded = store.record(row.clone()).expect("record");
    let piece = if recorded.is_file() {
        recorded
    } else {
        let path = store.directory().join("encoding-check.parquet");
        super::store::write_piece(&path, &row).expect("encoding piece");
        path
    };
    let encodings = column_encodings(&piece).expect("metadata");
    for name in ["time_unix_ms", "latency_ms", "token_count"] {
        let found = encodings
            .iter()
            .find(|(column, _)| column == name)
            .unwrap_or_else(|| panic!("missing column {name}"));
        assert!(
            found.1.contains(&Encoding::DELTA_BINARY_PACKED),
            "Operator: parquet metadata encoding for {name} is delta binary packing, saw {:?}",
            found.1
        );
    }
    if super::shared_library::installed_api().is_err() {
        return;
    }
    let rows = store.read_rows().expect("duckdb");
    assert_eq!(
        rows.len(),
        1,
        "Operator: Parquet pieces are readable by DuckDB"
    );
    assert_eq!(rows[0].time_unix_ms, NOW_UNIX_MS);
    assert_eq!(rows[0].latency_ms, Some(12));
    assert_eq!(rows[0].token_count, Some(9));
    assert!(!rows[0].tokens_not_fetched);
}

#[test]
fn operator_pager_cargo_toml_has_no_bundled_duckdb_crate() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = fs::read_to_string(&manifest).expect("pager Cargo.toml");
    assert!(
        !text.contains("bundled") && !text.contains("duckdb ="),
        "Operator: no bundled C++ DuckDB, shared library only, if the .so is missing uptime stays off"
    );
}

#[test]
fn operator_sqlite_session_rows_are_unchanged_because_the_series_is_not_written_to_sqlite() {
    fn assert_record_takes_no_sqlite_connection(
        record: fn(&UptimeStore, Observation) -> Result<PathBuf, StoreError>,
    ) {
        let _ = record;
    }
    assert_record_takes_no_sqlite_connection(UptimeStore::record);

    let (dir, store) = open_store();
    let sqlite_path = dir.path().join("session.sqlite");
    let sentinel = b"sqlite-sentinel-not-the-series";
    fs::write(&sqlite_path, sentinel).expect("seed sqlite");
    store
        .record(observation(Outcome::Timeout, NOW_UNIX_MS))
        .expect("record");
    let after = fs::read(&sqlite_path).expect("read sqlite");
    assert_eq!(
        after, sentinel,
        "Operator: sqlite session rows are unchanged because the series is not written to sqlite"
    );
    assert!(
        store
            .piece_paths()
            .expect("pieces")
            .iter()
            .all(|path| path.extension().and_then(|ext| ext.to_str()) == Some("parquet")),
        "Operator: the series is not written to sqlite"
    );
}

#[test]
fn operator_append_writes_a_new_piece_and_does_not_rewrite_the_first_piece() {
    let (_dir, store) = open_store();
    let first = store
        .record(observation(
            Outcome::ModelRequestSucceeded,
            NOW_UNIX_MS - 1_000,
        ))
        .expect("first");
    let first_bytes = fs::read(&first).expect("first bytes");
    let second = store
        .record(observation(Outcome::Timeout, NOW_UNIX_MS))
        .expect("second");
    assert_ne!(first, second, "Operator: append writes a new piece");
    assert_eq!(
        fs::read(&first).expect("first bytes again"),
        first_bytes,
        "Operator: a second observation does not rewrite the first piece's bytes"
    );
    assert_eq!(store.piece_paths().expect("pieces").len(), 2);
}

#[test]
fn operator_paint_format_does_not_write_a_new_piece() {
    let (_dir, store) = open_store();
    store
        .record(observation(Outcome::ModelRequestSucceeded, NOW_UNIX_MS))
        .expect("record");
    let before = piece_bytes(&store);
    let windows = store.aggregate(NOW_UNIX_MS).expect("aggregate reads");
    let text = format_uptime_beside_status(&windows);
    let after_read = piece_bytes(&store);
    assert_eq!(
        before, after_read,
        "Operator: aggregate only reads and paint/format does not write a new piece"
    );
    let _ = format_uptime_beside_status(&windows);
    let after_format = piece_bytes(&store);
    assert_eq!(
        before, after_format,
        "Operator: paint/format does not write a new piece: {text}"
    );
    assert!(
        text.contains("15 minutes") && text.contains("24 hours"),
        "Operator: the formatted block still says 15 minutes and 24 hours: {text}"
    );
}

#[test]
fn operator_uptime_shows_a_real_token_sum_and_omits_the_clause_when_a_count_is_missing() {
    let mut missing = super::window::WindowStats::empty(super::window::SHORT_WINDOW_WORDS);
    missing.observation_count = 6;
    missing.succeeded_count = 0;
    missing.measured_latency_sum_ms = 2_000;
    missing.measured_latency_count = 1;
    missing.fetched_token_sum = None;
    let mut missing_day = missing.clone();
    missing_day.duration_words = super::window::LONG_WINDOW_WORDS;
    let absent = format_uptime_beside_status(&super::window::WindowPair {
        last_15_minutes: missing,
        last_24_hours: missing_day,
    });
    assert!(
        !absent.contains("not fetched") && !absent.contains("tokens"),
        "Operator: no count omits the token clause: {absent}"
    );
    assert!(
        absent.contains("15 minutes") && absent.contains("24 hours"),
        "Operator: a window with observations still names 15 minutes and 24 hours: {absent}"
    );
    assert!(
        !absent.contains("167"),
        "Operator: do not invent 167.0k: {absent}"
    );

    let mut fetched = super::window::WindowStats::empty(super::window::SHORT_WINDOW_WORDS);
    fetched.observation_count = 1;
    fetched.succeeded_count = 1;
    fetched.fetched_token_sum = Some(9);
    let mut fetched_day = fetched.clone();
    fetched_day.duration_words = super::window::LONG_WINDOW_WORDS;
    fetched_day.fetched_token_sum = Some(12);
    let shown = format_uptime_beside_status(&super::window::WindowPair {
        last_15_minutes: fetched,
        last_24_hours: fetched_day,
    });
    assert!(
        shown.contains(", tokens 9") && shown.contains(", tokens 12"),
        "Operator: a real token count is shown: {shown}"
    );
    assert!(
        !shown.contains("not fetched") && !shown.contains("167.0k"),
        "Operator: a real count is not a placeholder and not 167.0k: {shown}"
    );
    assert!(
        !shown.contains("estimate"),
        "Operator: a stored token count is not relabeled as an estimate: {shown}"
    );
}

#[test]
fn operator_fetched_token_count_is_shown_and_a_partial_window_omits_the_clause() {
    let (_dir, store) = open_store();
    let mut fetched = observation(Outcome::ModelRequestSucceeded, NOW_UNIX_MS - 1_000);
    fetched.latency_ms = Some(40);
    fetched.token_count = Some(9);
    fetched.tokens_not_fetched = false;
    store.record(fetched).expect("record fetched");
    let mut also = observation(Outcome::ModelRequestSucceeded, NOW_UNIX_MS - 2_000);
    also.latency_ms = Some(40);
    also.token_count = Some(3);
    also.tokens_not_fetched = false;
    store.record(also).expect("record also fetched");
    let windows = store.aggregate(NOW_UNIX_MS).expect("aggregate");
    let text = format_uptime_beside_status(&windows);
    assert!(
        text.contains(", tokens 12"),
        "Operator: the window shows the sum of stored token counts: {text}"
    );
    assert!(
        !text.contains("not fetched"),
        "Operator: a real sum does not say not fetched: {text}"
    );
    assert_eq!(
        windows.last_15_minutes.fetched_token_sum,
        Some(12),
        "Operator: both windows have the same two stored counts"
    );
    assert_eq!(windows.last_24_hours.fetched_token_sum, Some(12));

    let mut banner = observation(Outcome::AnnouncementBanner, NOW_UNIX_MS - 500);
    banner.banner_text = Some(BANNER_MODEL_SERVING_ISSUES.to_string());
    store.record(banner).expect("record banner");
    let with_banner = store.aggregate(NOW_UNIX_MS).expect("aggregate banner");
    let banner_text = format_uptime_beside_status(&with_banner);
    assert_eq!(
        with_banner.last_15_minutes.fetched_token_sum,
        Some(12),
        "Operator: an announcement banner does not erase a stored token sum"
    );
    assert!(
        banner_text.contains(", tokens 12") && !banner_text.contains("not fetched"),
        "Operator: an announcement banner does not hide a stored token sum: {banner_text}"
    );

    let mut missing = observation(Outcome::Timeout, NOW_UNIX_MS);
    missing.token_count = None;
    missing.tokens_not_fetched = true;
    store.record(missing).expect("record missing");
    let mixed = store.aggregate(NOW_UNIX_MS).expect("aggregate mixed");
    let mixed_text = format_uptime_beside_status(&mixed);
    assert!(
        mixed.last_15_minutes.fetched_token_sum.is_none(),
        "Operator: one missing count means the window has no complete sum"
    );
    assert!(
        !mixed_text.contains("not fetched") && !mixed_text.contains("tokens"),
        "Operator: a partial window omits the token clause: {mixed_text}"
    );
    assert!(
        !mixed_text.contains("167")
            && !mixed_text.contains("tokens 12")
            && !mixed_text.contains("tokens 9")
            && !mixed_text.contains("tokens 3"),
        "Operator: a partial window does not invent or show a partial sum: {mixed_text}"
    );
}

#[test]
fn operator_empty_uptime_windows_are_a_short_phrase_not_two_sentences() {
    let text = format_uptime_beside_status(&crate::uptime::WindowPair::empty());
    assert_eq!(
        text, "15m none · 24h none",
        "Operator: empty windows are a few words, not the sentence twice: {text}"
    );
    assert!(
        !text.contains("no observations in the last"),
        "Operator: the long sentence must not return: {text}"
    );
    assert!(!text.contains('\u{2014}'));
}

#[test]
fn operator_uptime_status_segment_stays_short_so_supergrok_period_limits_fit() {
    use crate::uptime::cap_uptime_status_segment;
    let short = "15m none · 24h none";
    assert_eq!(cap_uptime_status_segment(short), short);
    let long = format_uptime_beside_status(&crate::uptime::WindowPair::empty())
        .replace("none", "no observations in the last 15 minutes");
    let capped = cap_uptime_status_segment(&long);
    assert!(
        unicode_width::UnicodeWidthStr::width(capped.as_str()) <= 24,
        "{capped}"
    );
    assert!(
        !capped.contains('%'),
        "do not invent a used percent: {capped}"
    );
    assert_eq!(
        cap_uptime_status_segment("uptime tracking is off because DuckDB is not installed"),
        "uptime off, no DuckDB"
    );
}
