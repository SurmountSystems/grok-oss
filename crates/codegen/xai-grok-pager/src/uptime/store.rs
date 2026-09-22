//! Append-only Parquet pieces for local uptime observations.
//! The installed DuckDB shared library reads the directory.
//! A missing library leaves tracking off and writes no piece.
//! Nothing here writes sqlite or calls xAI.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parquet::basic::{Compression, Encoding};
use parquet::data_type::{BoolType, ByteArray, ByteArrayType, DataType, Int64Type};
use parquet::file::properties::{WriterProperties, WriterVersion};
#[cfg(test)]
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::writer::{
    SerializedColumnWriter, SerializedFileWriter, SerializedRowGroupWriter,
};
use parquet::schema::parser::parse_message_type;
use parquet::schema::types::{ColumnPath, TypePtr};

use super::window::{
    LONG_WINDOW_MS, LONG_WINDOW_WORDS, SHORT_WINDOW_MS, SHORT_WINDOW_WORDS, WindowPair, WindowStats,
};

/// Banner the product already shows when model serving is failing.
pub const BANNER_MODEL_SERVING_ISSUES: &str =
    "We're currently experiencing issues serving our models";

/// Banner the product already shows for a datacenter incident.
pub const BANNER_DATACENTER_INCIDENT: &str = "A datacenter incident is affecting all Grok models";

const SCHEMA: &str = "
message uptime_observation {
  REQUIRED INT64 time_unix_ms;
  REQUIRED BYTE_ARRAY outcome (UTF8);
  OPTIONAL INT64 latency_ms;
  OPTIONAL INT64 token_count;
  REQUIRED BOOLEAN tokens_not_fetched;
  REQUIRED BYTE_ARRAY model_id (UTF8);
  REQUIRED BYTE_ARRAY local_session_id (UTF8);
  OPTIONAL BYTE_ARRAY banner_text (UTF8);
}
";

const NUMERIC_COLUMNS: [&str; 3] = ["time_unix_ms", "latency_ms", "token_count"];

static PIECE_SEQ: AtomicU64 = AtomicU64::new(1);

/// What finished, as stored in the `outcome` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    ModelRequestSucceeded,
    Http500,
    Timeout,
    RepeatingSentenceStop,
    AnnouncementBanner,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelRequestSucceeded => "model_request_succeeded",
            Self::Http500 => "http_500",
            Self::Timeout => "timeout",
            Self::RepeatingSentenceStop => "repeating_sentence_stop",
            Self::AnnouncementBanner => "announcement_banner",
        }
    }

    pub fn is_success(self) -> bool {
        matches!(self, Self::ModelRequestSucceeded)
    }
}

/// One local observation. `local_session_id` is an argument. This type does
/// not read the session module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub time_unix_ms: i64,
    pub outcome: Outcome,
    /// Present only when latency was actually measured.
    pub latency_ms: Option<i64>,
    /// Present only when tokens were fetched. Ignored when `tokens_not_fetched`.
    pub token_count: Option<i64>,
    pub tokens_not_fetched: bool,
    pub model_id: String,
    pub local_session_id: String,
    pub banner_text: Option<String>,
}

/// One row read back through DuckDB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRow {
    pub time_unix_ms: i64,
    pub outcome: String,
    pub latency_ms: Option<i64>,
    pub token_count: Option<i64>,
    pub tokens_not_fetched: bool,
    pub model_id: String,
    pub local_session_id: String,
    pub banner_text: Option<String>,
}

#[derive(Debug)]
pub struct StoreError {
    pub message: String,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for StoreError {}

fn err(message: impl Into<String>) -> StoreError {
    StoreError {
        message: message.into(),
    }
}

/// Directory of Parquet pieces under a grok home. Callers pass the home.
pub fn uptime_dir(grok_home: &Path) -> PathBuf {
    grok_home.join("uptime")
}

/// Status-line text for this uptime directory. A missing shared library,
/// or a failed open or aggregate, shows tracking off and invents no rows.
pub fn text_beside_status(dir: &Path, now_unix_ms: i64) -> String {
    let text = if super::shared_library::installed_api().is_err() {
        super::window::format_tracking_off()
    } else {
        match UptimeStore::open(dir).and_then(|store| store.aggregate(now_unix_ms)) {
            Ok(windows) => super::window::format_uptime_beside_status(&windows),
            Err(_) => super::window::format_tracking_off(),
        }
    };
    append_extra_request_count(text)
}

/// The status line reads the extra-request counter. Zero leaves the sentence
/// unchanged. This does not send a request.
fn append_extra_request_count(text: String) -> String {
    let extra = super::extra_api_requests();
    if extra == 0 {
        text
    } else {
        format!("{text}; extra API requests {extra}")
    }
}

/// Recognized announcement copy, if this text is one of the two banners.
/// Unit-test builds do not record banners, so this helper is not compiled there.
#[cfg(not(test))]
pub fn outcome_from_banner_text(text: &str) -> Option<Outcome> {
    if text.contains(BANNER_MODEL_SERVING_ISSUES) || text.contains(BANNER_DATACENTER_INCIDENT) {
        Some(Outcome::AnnouncementBanner)
    } else {
        None
    }
}

/// Writer properties. Numeric columns ask for delta binary packing.
/// Dictionary encoding is off so the file metadata can record that encoding.
pub fn writer_properties() -> WriterProperties {
    let mut builder = WriterProperties::builder()
        .set_writer_version(WriterVersion::PARQUET_2_0)
        .set_compression(Compression::UNCOMPRESSED)
        .set_dictionary_enabled(false);
    for name in NUMERIC_COLUMNS {
        builder =
            builder.set_column_encoding(ColumnPath::from(name), Encoding::DELTA_BINARY_PACKED);
    }
    builder.build()
}

/// Append-only directory of `*.parquet` pieces.
#[derive(Debug, Clone)]
pub struct UptimeStore {
    dir: PathBuf,
}

impl UptimeStore {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|e| err(format!("create uptime directory: {e}")))?;
        Ok(Self { dir })
    }

    pub fn directory(&self) -> &Path {
        &self.dir
    }

    /// Write one new piece. Does not read or rewrite older pieces.
    /// When the installed DuckDB shared library is missing, returns this
    /// directory and does not create a piece.
    pub fn record(&self, observation: Observation) -> Result<PathBuf, StoreError> {
        super::send_synthetic_probe();
        if super::shared_library::installed_api().is_err() {
            return Ok(self.dir.clone());
        }
        let name = next_piece_name(observation.time_unix_ms);
        let final_path = self.dir.join(&name);
        let tmp_path = self.dir.join(format!("{name}.tmp"));
        if let Err(e) = write_piece(&tmp_path, &observation) {
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }
        fs::rename(&tmp_path, &final_path).map_err(|e| err(format!("rename piece: {e}")))?;
        Ok(final_path)
    }

    pub fn piece_paths(&self) -> Result<Vec<PathBuf>, StoreError> {
        list_piece_paths(&self.dir)
    }

    /// Read every piece through DuckDB `read_parquet`. Does not write.
    pub fn read_rows(&self) -> Result<Vec<StoredRow>, StoreError> {
        read_rows_through_duckdb(&self.dir)
    }

    /// Read pieces and count the two windows. Does not write.
    pub fn aggregate(&self, now_unix_ms: i64) -> Result<WindowPair, StoreError> {
        let rows = self.read_rows()?;
        if rows.is_empty() {
            return Ok(WindowPair::empty());
        }
        Ok(WindowPair {
            last_15_minutes: summarize(SHORT_WINDOW_WORDS, SHORT_WINDOW_MS, now_unix_ms, &rows),
            last_24_hours: summarize(LONG_WINDOW_WORDS, LONG_WINDOW_MS, now_unix_ms, &rows),
        })
    }
}

/// `/announcements hide` is a UI hide. This does not delete Parquet pieces.
pub fn hide_announcement_keeps_row(store: &UptimeStore) -> bool {
    let _ = store.directory();
    true
}

/// Record a finished model request, HTTP 500, timeout, or repeating-sentence stop.
/// `token_count` is `None` when tokens were not fetched. This does not invent a number.
/// When the installed DuckDB shared library is missing, returns Ok and writes no piece.
/// The pager calls this from the grok-oss binary. Unit tests do not.
#[cfg(not(test))]
pub fn record_completed_observation(
    dir: &Path,
    local_session_id: &str,
    model_id: &str,
    outcome: Outcome,
    latency_ms: Option<i64>,
    token_count: Option<i64>,
    time_unix_ms: i64,
) -> Result<(), StoreError> {
    if super::shared_library::installed_api().is_err() {
        return Ok(());
    }
    let store = UptimeStore::open(dir)?;
    store.record(Observation {
        time_unix_ms,
        outcome,
        latency_ms,
        token_count,
        tokens_not_fetched: token_count.is_none(),
        model_id: model_id.to_string(),
        local_session_id: local_session_id.to_string(),
        banner_text: None,
    })?;
    Ok(())
}

/// Store a banner the product already showed, when the text is one of the two
/// known sentences. Returns false and writes nothing for other text.
/// When the installed DuckDB shared library is missing, returns false and writes no piece.
/// The pager calls this from the grok-oss binary. Unit tests do not.
#[cfg(not(test))]
pub fn record_announcement_if_recognized(
    dir: &Path,
    local_session_id: &str,
    banner_text: &str,
    time_unix_ms: i64,
) -> Result<bool, StoreError> {
    if outcome_from_banner_text(banner_text).is_none() {
        return Ok(false);
    }
    if super::shared_library::installed_api().is_err() {
        return Ok(false);
    }
    let store = UptimeStore::open(dir)?;
    store.record(Observation {
        time_unix_ms,
        outcome: Outcome::AnnouncementBanner,
        latency_ms: None,
        token_count: None,
        tokens_not_fetched: true,
        model_id: String::new(),
        local_session_id: local_session_id.to_string(),
        banner_text: Some(banner_text.to_string()),
    })?;
    Ok(true)
}

pub fn read_rows_through_duckdb(dir: &Path) -> Result<Vec<StoredRow>, StoreError> {
    let paths = UptimeStore {
        dir: dir.to_path_buf(),
    }
    .piece_paths()?;
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let api = match super::shared_library::installed_api() {
        Ok(api) => api,
        Err(super::shared_library::QueryFail::LibraryMissing) => return Ok(Vec::new()),
        Err(super::shared_library::QueryFail::Unreadable(message)) => return Err(err(message)),
    };
    let list = parquet_list_sql(&paths);
    let sql = format!(
        "SELECT time_unix_ms, outcome, latency_ms, token_count, tokens_not_fetched, \
         model_id, local_session_id, banner_text \
         FROM read_parquet([{list}], hive_partitioning = false, union_by_name = true)"
    );
    let raw_rows = match super::shared_library::query_uptime_select(&api, &sql) {
        Ok(rows) => rows,
        Err(super::shared_library::QueryFail::LibraryMissing) => return Ok(Vec::new()),
        Err(super::shared_library::QueryFail::Unreadable(message)) => return Err(err(message)),
    };
    Ok(raw_rows.into_iter().map(stored_from_raw).collect())
}

fn stored_from_raw(row: super::shared_library::RawRow) -> StoredRow {
    StoredRow {
        time_unix_ms: row.time_unix_ms,
        outcome: row.outcome,
        latency_ms: row.latency_ms,
        token_count: row.token_count,
        tokens_not_fetched: row.tokens_not_fetched,
        model_id: row.model_id,
        local_session_id: row.local_session_id,
        banner_text: row.banner_text,
    }
}

/// Encodings recorded in one piece's column-chunk metadata.
/// Only the uptime unit tests read this metadata.
#[cfg(test)]
pub fn column_encodings(path: &Path) -> Result<Vec<(String, Vec<Encoding>)>, StoreError> {
    let file = File::open(path).map_err(|e| err(format!("open piece: {e}")))?;
    let reader = SerializedFileReader::new(file).map_err(|e| err(format!("read piece: {e}")))?;
    let metadata = reader.metadata();
    let mut found = Vec::new();
    for row_group in metadata.row_groups() {
        for column in row_group.columns() {
            found.push((column.column_path().string(), column.encodings().to_vec()));
        }
    }
    Ok(found)
}

fn stored_outcome_is_success(outcome: &str) -> bool {
    let parsed = if outcome == Outcome::ModelRequestSucceeded.as_str() {
        Some(Outcome::ModelRequestSucceeded)
    } else if outcome == Outcome::Http500.as_str() {
        Some(Outcome::Http500)
    } else if outcome == Outcome::Timeout.as_str() {
        Some(Outcome::Timeout)
    } else if outcome == Outcome::RepeatingSentenceStop.as_str() {
        Some(Outcome::RepeatingSentenceStop)
    } else if outcome == Outcome::AnnouncementBanner.as_str() {
        Some(Outcome::AnnouncementBanner)
    } else {
        None
    };
    parsed.is_some_and(Outcome::is_success)
}

fn summarize(
    duration_words: &'static str,
    span_ms: i64,
    now_unix_ms: i64,
    rows: &[StoredRow],
) -> WindowStats {
    let start = now_unix_ms.saturating_sub(span_ms);
    let mut stats = WindowStats::empty(duration_words);
    for row in rows {
        if row.time_unix_ms < start || row.time_unix_ms > now_unix_ms {
            continue;
        }
        stats.observation_count += 1;
        if stored_outcome_is_success(&row.outcome) {
            stats.succeeded_count += 1;
        }
        if row.outcome == Outcome::Http500.as_str() {
            stats.http_500_count += 1;
        }
        if let Some(latency) = row.latency_ms {
            stats.measured_latency_sum_ms += i128::from(latency);
            stats.measured_latency_count += 1;
        }
        if row.tokens_not_fetched || row.token_count.is_none() {
            stats.tokens_not_fetched = true;
        }
    }
    stats
}

fn next_piece_name(time_unix_ms: i64) -> String {
    let seq = PIECE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{time_unix_ms}-{seq:016}.parquet")
}

fn schema() -> Result<TypePtr, StoreError> {
    parse_message_type(SCHEMA)
        .map(Arc::new)
        .map_err(|e| err(format!("uptime schema: {e}")))
}

fn stored_token(observation: &Observation) -> Option<i64> {
    if observation.tokens_not_fetched {
        None
    } else {
        observation.token_count
    }
}

pub(in crate::uptime) fn write_piece(
    path: &Path,
    observation: &Observation,
) -> Result<(), StoreError> {
    let file = File::create(path).map_err(|e| err(format!("create piece: {e}")))?;
    let props = Arc::new(writer_properties());
    let mut writer = SerializedFileWriter::new(file, schema()?, props)
        .map_err(|e| err(format!("parquet writer: {e}")))?;
    let mut row_group = writer
        .next_row_group()
        .map_err(|e| err(format!("parquet row group: {e}")))?;
    write_i64_required(&mut row_group, observation.time_unix_ms)?;
    write_utf8_required(&mut row_group, observation.outcome.as_str())?;
    write_i64_optional(&mut row_group, observation.latency_ms)?;
    let token = stored_token(observation);
    write_i64_optional(&mut row_group, token)?;
    write_bool_required(&mut row_group, token.is_none())?;
    write_utf8_required(&mut row_group, &observation.model_id)?;
    write_utf8_required(&mut row_group, &observation.local_session_id)?;
    write_utf8_optional(&mut row_group, observation.banner_text.as_deref())?;
    row_group
        .close()
        .map_err(|e| err(format!("close row group: {e}")))?;
    writer
        .close()
        .map_err(|e| err(format!("close parquet: {e}")))?;
    Ok(())
}

fn list_piece_paths(dir: &Path) -> Result<Vec<PathBuf>, StoreError> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(paths),
        Err(e) => return Err(err(format!("read uptime directory: {e}"))),
    };
    for entry in entries {
        let entry = entry.map_err(|e| err(format!("read uptime entry: {e}")))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("parquet") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

// `SerializedColumnWriter` borrows this `&mut`. That borrow is shorter than
// the row group's buffer lifetime, so the two lifetimes stay distinct.
fn next_column<'a, 'b>(
    row_group: &'a mut SerializedRowGroupWriter<'b, File>,
) -> Result<SerializedColumnWriter<'a>, StoreError> {
    row_group
        .next_column()
        .map_err(|e| err(format!("parquet column: {e}")))?
        .ok_or_else(|| err("parquet writer ran out of columns"))
}

fn write_i64_required(
    row_group: &mut SerializedRowGroupWriter<File>,
    value: i64,
) -> Result<(), StoreError> {
    let mut column = next_column(row_group)?;
    write_values::<Int64Type>(&mut column, &[value], None)?;
    column
        .close()
        .map_err(|e| err(format!("close i64 column: {e}")))?;
    Ok(())
}

fn write_i64_optional(
    row_group: &mut SerializedRowGroupWriter<File>,
    value: Option<i64>,
) -> Result<(), StoreError> {
    let mut column = next_column(row_group)?;
    match value {
        Some(value) => write_values::<Int64Type>(&mut column, &[value], Some(&[1]))?,
        None => write_values::<Int64Type>(&mut column, &[], Some(&[0]))?,
    }
    column
        .close()
        .map_err(|e| err(format!("close optional i64 column: {e}")))?;
    Ok(())
}

fn write_bool_required(
    row_group: &mut SerializedRowGroupWriter<File>,
    value: bool,
) -> Result<(), StoreError> {
    let mut column = next_column(row_group)?;
    write_values::<BoolType>(&mut column, &[value], None)?;
    column
        .close()
        .map_err(|e| err(format!("close bool column: {e}")))?;
    Ok(())
}

fn write_utf8_required(
    row_group: &mut SerializedRowGroupWriter<File>,
    value: &str,
) -> Result<(), StoreError> {
    let mut column = next_column(row_group)?;
    let bytes = ByteArray::from(value.as_bytes().to_vec());
    write_values::<ByteArrayType>(&mut column, &[bytes], None)?;
    column
        .close()
        .map_err(|e| err(format!("close utf8 column: {e}")))?;
    Ok(())
}

fn write_utf8_optional(
    row_group: &mut SerializedRowGroupWriter<File>,
    value: Option<&str>,
) -> Result<(), StoreError> {
    let mut column = next_column(row_group)?;
    match value {
        Some(value) => {
            let bytes = ByteArray::from(value.as_bytes().to_vec());
            write_values::<ByteArrayType>(&mut column, &[bytes], Some(&[1]))?;
        }
        None => write_values::<ByteArrayType>(&mut column, &[], Some(&[0]))?,
    }
    column
        .close()
        .map_err(|e| err(format!("close optional utf8 column: {e}")))?;
    Ok(())
}

fn write_values<T: DataType>(
    column: &mut SerializedColumnWriter<'_>,
    values: &[T::T],
    def_levels: Option<&[i16]>,
) -> Result<(), StoreError> {
    column
        .typed::<T>()
        .write_batch(values, def_levels, None)
        .map_err(|e| err(format!("write parquet column: {e}")))?;
    Ok(())
}

fn parquet_list_sql(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| sql_quote(&path.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn sql_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
