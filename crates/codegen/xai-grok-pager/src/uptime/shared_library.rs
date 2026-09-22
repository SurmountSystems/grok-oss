//! Load the installed DuckDB shared library when a query needs it.
//!
//! The `duckdb` crate is not a dependency. Its `parquet` feature turns on
//! `bundled`, which compiles C++. Without that feature, `libduckdb-sys` still
//! emits `cargo:rustc-link-lib=dylib=duckdb`, and the dynamic linker then
//! aborts process start when the shared library is absent. `dlopen` /
//! `LoadLibraryA` fail closed instead: tracking stays off, and grok-oss keeps
//! running.
//!
//! The `duckdb_result` layout matches libduckdb-sys 1.10505.0 (DuckDB 1.5.5).
//! A loaded library whose version string is not 1.5 is refused so a mismatched
//! ABI is not called.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::OnceLock;

/// One `SELECT` row. Column order is the uptime `read_parquet` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::uptime) struct RawRow {
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
pub(in crate::uptime) enum QueryFail {
    LibraryMissing,
    Unreadable(String),
}

#[cfg(target_pointer_width = "64")]
#[derive(Clone, Copy)]
pub(in crate::uptime) struct DuckApi {
    _handle: usize,
    create_config: CreateConfig,
    set_config: SetConfig,
    destroy_config: DestroyConfig,
    open_ext: OpenExt,
    connect: Connect,
    disconnect: Disconnect,
    close: Close,
    query: QueryFn,
    destroy_result: DestroyResult,
    row_count: RowCount,
    column_count: ColumnCount,
    value_int64: ValueInt64,
    value_varchar: ValueVarchar,
    value_boolean: ValueBoolean,
    value_is_null: ValueIsNull,
    free: FreeFn,
    result_error: ResultError,
    library_version: LibraryVersion,
}

#[cfg(not(target_pointer_width = "64"))]
#[derive(Clone, Copy)]
pub(in crate::uptime) struct DuckApi;

#[cfg(target_os = "linux")]
pub(in crate::uptime) fn default_library_names() -> &'static [&'static str] {
    &["libduckdb.so", "libduckdb.so.1"]
}

#[cfg(target_os = "macos")]
pub(in crate::uptime) fn default_library_names() -> &'static [&'static str] {
    &["libduckdb.dylib"]
}

#[cfg(windows)]
pub(in crate::uptime) fn default_library_names() -> &'static [&'static str] {
    &["duckdb.dll"]
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(in crate::uptime) fn default_library_names() -> &'static [&'static str] {
    &[]
}

pub(in crate::uptime) fn installed_api() -> Result<DuckApi, ()> {
    static CELL: OnceLock<Result<DuckApi, ()>> = OnceLock::new();
    *CELL.get_or_init(|| load_uncached(default_library_names()))
}

pub(in crate::uptime) fn load_uncached(names: &[&str]) -> Result<DuckApi, ()> {
    #[cfg(target_pointer_width = "64")]
    {
        load_uncached_wide(names)
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        let _ = names;
        Err(())
    }
}

pub(in crate::uptime) fn query_uptime_select(
    api: &DuckApi,
    sql: &str,
) -> Result<Vec<RawRow>, QueryFail> {
    #[cfg(target_pointer_width = "64")]
    {
        query_uptime_select_wide(api, sql)
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        let _ = (api, sql);
        Err(QueryFail::LibraryMissing)
    }
}

#[cfg(target_pointer_width = "64")]
const MAX_ROWS: u64 = 1_000_000;

#[cfg(target_pointer_width = "64")]
const DUCKDB_SUCCESS: u32 = 0;

#[cfg(target_pointer_width = "64")]
#[repr(C)]
struct DuckResult {
    deprecated_column_count: u64,
    deprecated_row_count: u64,
    deprecated_rows_changed: u64,
    deprecated_columns: *mut c_void,
    deprecated_error_message: *mut c_char,
    internal_data: *mut c_void,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<DuckResult>() == 48);
    assert!(std::mem::align_of::<DuckResult>() == 8);
};

#[cfg(target_pointer_width = "64")]
impl DuckResult {
    fn empty() -> Self {
        Self {
            deprecated_column_count: 0,
            deprecated_row_count: 0,
            deprecated_rows_changed: 0,
            deprecated_columns: ptr::null_mut(),
            deprecated_error_message: ptr::null_mut(),
            internal_data: ptr::null_mut(),
        }
    }
}

#[cfg(target_pointer_width = "64")]
type CreateConfig = unsafe extern "C" fn(*mut *mut c_void) -> u32;
#[cfg(target_pointer_width = "64")]
type SetConfig = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> u32;
#[cfg(target_pointer_width = "64")]
type DestroyConfig = unsafe extern "C" fn(*mut *mut c_void);
#[cfg(target_pointer_width = "64")]
type OpenExt =
    unsafe extern "C" fn(*const c_char, *mut *mut c_void, *mut c_void, *mut *mut c_char) -> u32;
#[cfg(target_pointer_width = "64")]
type Connect = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> u32;
#[cfg(target_pointer_width = "64")]
type Disconnect = unsafe extern "C" fn(*mut *mut c_void);
#[cfg(target_pointer_width = "64")]
type Close = unsafe extern "C" fn(*mut *mut c_void);
#[cfg(target_pointer_width = "64")]
type QueryFn = unsafe extern "C" fn(*mut c_void, *const c_char, *mut DuckResult) -> u32;
#[cfg(target_pointer_width = "64")]
type DestroyResult = unsafe extern "C" fn(*mut DuckResult);
#[cfg(target_pointer_width = "64")]
type RowCount = unsafe extern "C" fn(*mut DuckResult) -> u64;
#[cfg(target_pointer_width = "64")]
type ColumnCount = unsafe extern "C" fn(*mut DuckResult) -> u64;
#[cfg(target_pointer_width = "64")]
type ValueInt64 = unsafe extern "C" fn(*mut DuckResult, u64, u64) -> i64;
#[cfg(target_pointer_width = "64")]
type ValueVarchar = unsafe extern "C" fn(*mut DuckResult, u64, u64) -> *mut c_char;
#[cfg(target_pointer_width = "64")]
type ValueBoolean = unsafe extern "C" fn(*mut DuckResult, u64, u64) -> bool;
#[cfg(target_pointer_width = "64")]
type ValueIsNull = unsafe extern "C" fn(*mut DuckResult, u64, u64) -> bool;
#[cfg(target_pointer_width = "64")]
type FreeFn = unsafe extern "C" fn(*mut c_void);
#[cfg(target_pointer_width = "64")]
type ResultError = unsafe extern "C" fn(*mut DuckResult) -> *const c_char;
#[cfg(target_pointer_width = "64")]
type LibraryVersion = unsafe extern "C" fn() -> *const c_char;

#[cfg(target_pointer_width = "64")]
fn load_uncached_wide(names: &[&str]) -> Result<DuckApi, ()> {
    for name in names {
        let Some(handle) = open_library(name) else {
            continue;
        };
        match bind(handle) {
            Ok(api) if version_is_1_5(&api) => return Ok(api),
            Ok(_) | Err(()) => close_library(handle),
        }
    }
    Err(())
}

#[cfg(target_pointer_width = "64")]
fn version_is_1_5(api: &DuckApi) -> bool {
    // SAFETY: `duckdb_library_version` returns a process-lifetime C string.
    let ptr = unsafe { (api.library_version)() };
    if ptr.is_null() {
        return false;
    }
    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
    text.starts_with("v1.5.") || text.starts_with("1.5.")
}

#[cfg(target_pointer_width = "64")]
fn bind(handle: usize) -> Result<DuckApi, ()> {
    Ok(DuckApi {
        _handle: handle,
        create_config: sym(handle, c"duckdb_create_config")?,
        set_config: sym(handle, c"duckdb_set_config")?,
        destroy_config: sym(handle, c"duckdb_destroy_config")?,
        open_ext: sym(handle, c"duckdb_open_ext")?,
        connect: sym(handle, c"duckdb_connect")?,
        disconnect: sym(handle, c"duckdb_disconnect")?,
        close: sym(handle, c"duckdb_close")?,
        query: sym(handle, c"duckdb_query")?,
        destroy_result: sym(handle, c"duckdb_destroy_result")?,
        row_count: sym(handle, c"duckdb_row_count")?,
        column_count: sym(handle, c"duckdb_column_count")?,
        value_int64: sym(handle, c"duckdb_value_int64")?,
        value_varchar: sym(handle, c"duckdb_value_varchar")?,
        value_boolean: sym(handle, c"duckdb_value_boolean")?,
        value_is_null: sym(handle, c"duckdb_value_is_null")?,
        free: sym(handle, c"duckdb_free")?,
        result_error: sym(handle, c"duckdb_result_error")?,
        library_version: sym(handle, c"duckdb_library_version")?,
    })
}

#[cfg(target_pointer_width = "64")]
fn sym<T>(handle: usize, name: &CStr) -> Result<T, ()> {
    let ptr = raw_symbol(handle, name);
    if ptr.is_null() {
        return Err(());
    }
    // SAFETY: `dlsym` / `GetProcAddress` returned a function with this C ABI.
    // `T` is a function pointer the same width as `*mut c_void`.
    Ok(unsafe { std::mem::transmute_copy(&ptr) })
}

#[cfg(target_pointer_width = "64")]
fn query_uptime_select_wide(api: &DuckApi, sql: &str) -> Result<Vec<RawRow>, QueryFail> {
    let sql = CString::new(sql)
        .map_err(|_| QueryFail::Unreadable("uptime sql contains a nul".to_string()))?;
    let mut config = ptr::null_mut();
    // SAFETY: `out_config` is a local the library writes. The handle is freed below.
    let created = unsafe { (api.create_config)(&mut config) };
    if created != DUCKDB_SUCCESS || config.is_null() {
        return Err(QueryFail::Unreadable("duckdb config failed".to_string()));
    }
    if let Err(message) = disable_extension_network(api, config) {
        destroy_config(api, &mut config);
        return Err(QueryFail::Unreadable(message));
    }
    let mut db = ptr::null_mut();
    let mut open_error = ptr::null_mut();
    // SAFETY: null path opens an in-memory database. `open_error` is freed with `duckdb_free`.
    let opened = unsafe { (api.open_ext)(ptr::null(), &mut db, config, &mut open_error) };
    destroy_config(api, &mut config);
    if opened != DUCKDB_SUCCESS || db.is_null() {
        let message =
            take_c_string(api, open_error).unwrap_or_else(|| "duckdb open failed".to_string());
        return Err(QueryFail::Unreadable(message));
    }
    let mut conn = ptr::null_mut();
    // SAFETY: `db` came from `duckdb_open_ext` and is closed on every exit path.
    let connected = unsafe { (api.connect)(db, &mut conn) };
    if connected != DUCKDB_SUCCESS || conn.is_null() {
        close_db(api, &mut db);
        return Err(QueryFail::Unreadable("duckdb connect failed".to_string()));
    }
    let mut result = DuckResult::empty();
    // SAFETY: `conn` is live. `duckdb_destroy_result` runs after `duckdb_query` returns,
    // including when the query fails, which is what the C API requires.
    let state = unsafe { (api.query)(conn, sql.as_ptr(), &mut result) };
    if state != DUCKDB_SUCCESS {
        let message = result_message(api, &mut result);
        destroy_result(api, &mut result);
        disconnect(api, &mut conn);
        close_db(api, &mut db);
        return Err(QueryFail::Unreadable(message));
    }
    let decoded = decode_rows(api, &mut result);
    destroy_result(api, &mut result);
    disconnect(api, &mut conn);
    close_db(api, &mut db);
    decoded
}

#[cfg(target_pointer_width = "64")]
fn disable_extension_network(api: &DuckApi, config: *mut c_void) -> Result<(), String> {
    for name in [
        c"autoinstall_known_extensions",
        c"autoload_known_extensions",
    ] {
        // SAFETY: `config` is a live config object. The name and value are nul-terminated literals.
        let state = unsafe { (api.set_config)(config, name.as_ptr(), c"false".as_ptr()) };
        if state != DUCKDB_SUCCESS {
            return Err(format!("duckdb refused setting {}", name.to_string_lossy()));
        }
    }
    Ok(())
}

#[cfg(target_pointer_width = "64")]
fn decode_rows(api: &DuckApi, result: &mut DuckResult) -> Result<Vec<RawRow>, QueryFail> {
    // SAFETY: `result` was filled by a successful `duckdb_query` and is not yet destroyed.
    let columns = unsafe { (api.column_count)(result) };
    let rows = unsafe { (api.row_count)(result) };
    if columns != 8 {
        return Err(QueryFail::Unreadable(format!(
            "duckdb returned {columns} columns, expected 8"
        )));
    }
    if rows > MAX_ROWS {
        return Err(QueryFail::Unreadable(format!(
            "duckdb returned {rows} rows"
        )));
    }
    let mut out = Vec::with_capacity(usize::try_from(rows).unwrap_or(0));
    for row in 0..rows {
        out.push(RawRow {
            time_unix_ms: unsafe { (api.value_int64)(result, 0, row) },
            outcome: varchar(api, result, 1, row)?,
            latency_ms: optional_i64(api, result, 2, row),
            token_count: optional_i64(api, result, 3, row),
            tokens_not_fetched: unsafe { (api.value_boolean)(result, 4, row) },
            model_id: varchar(api, result, 5, row)?,
            local_session_id: varchar(api, result, 6, row)?,
            banner_text: optional_varchar(api, result, 7, row)?,
        });
    }
    Ok(out)
}

#[cfg(target_pointer_width = "64")]
fn optional_i64(api: &DuckApi, result: &mut DuckResult, col: u64, row: u64) -> Option<i64> {
    // SAFETY: column and row are inside the result just counted.
    if unsafe { (api.value_is_null)(result, col, row) } {
        None
    } else {
        Some(unsafe { (api.value_int64)(result, col, row) })
    }
}

#[cfg(target_pointer_width = "64")]
fn optional_varchar(
    api: &DuckApi,
    result: &mut DuckResult,
    col: u64,
    row: u64,
) -> Result<Option<String>, QueryFail> {
    // SAFETY: same result object as `decode_rows`.
    if unsafe { (api.value_is_null)(result, col, row) } {
        Ok(None)
    } else {
        Ok(Some(varchar(api, result, col, row)?))
    }
}

#[cfg(target_pointer_width = "64")]
fn varchar(
    api: &DuckApi,
    result: &mut DuckResult,
    col: u64,
    row: u64,
) -> Result<String, QueryFail> {
    // SAFETY: `duckdb_value_varchar` returns memory that must be released with `duckdb_free`.
    let ptr = unsafe { (api.value_varchar)(result, col, row) };
    if ptr.is_null() {
        return Ok(String::new());
    }
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { (api.free)(ptr.cast()) };
    Ok(text)
}

#[cfg(target_pointer_width = "64")]
fn result_message(api: &DuckApi, result: &mut DuckResult) -> String {
    // SAFETY: the error pointer is owned by the result and must not be freed here.
    let ptr = unsafe { (api.result_error)(result) };
    if ptr.is_null() {
        "duckdb query failed".to_string()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(target_pointer_width = "64")]
fn take_c_string(api: &DuckApi, ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: DuckDB documents this error pointer as `duckdb_free` memory.
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { (api.free)(ptr.cast()) };
    Some(text)
}

#[cfg(target_pointer_width = "64")]
fn destroy_config(api: &DuckApi, config: &mut *mut c_void) {
    if config.is_null() {
        return;
    }
    // SAFETY: `config` was allocated by `duckdb_create_config`.
    unsafe { (api.destroy_config)(config) };
    *config = ptr::null_mut();
}

#[cfg(target_pointer_width = "64")]
fn destroy_result(api: &DuckApi, result: &mut DuckResult) {
    // SAFETY: `result` was passed to `duckdb_query`, which requires this call.
    unsafe { (api.destroy_result)(result) };
}

#[cfg(target_pointer_width = "64")]
fn disconnect(api: &DuckApi, conn: &mut *mut c_void) {
    if conn.is_null() {
        return;
    }
    // SAFETY: `conn` was allocated by `duckdb_connect`.
    unsafe { (api.disconnect)(conn) };
    *conn = ptr::null_mut();
}

#[cfg(target_pointer_width = "64")]
fn close_db(api: &DuckApi, db: &mut *mut c_void) {
    if db.is_null() {
        return;
    }
    // SAFETY: `db` was allocated by `duckdb_open_ext`.
    unsafe { (api.close)(db) };
    *db = ptr::null_mut();
}

#[cfg(all(target_pointer_width = "64", unix))]
fn open_library(name: &str) -> Option<usize> {
    let mut bytes = Vec::with_capacity(name.len() + 1);
    bytes.extend_from_slice(name.as_bytes());
    bytes.push(0);
    // SAFETY: the path is a local nul-terminated byte buffer. A null handle is a failed load,
    // not a process abort. The successful handle is kept for the process, or closed if bind fails.
    let handle = unsafe { libc::dlopen(bytes.as_ptr().cast(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        None
    } else {
        Some(handle as usize)
    }
}

#[cfg(all(target_pointer_width = "64", unix))]
fn raw_symbol(handle: usize, name: &CStr) -> *mut c_void {
    // SAFETY: `handle` is a live `dlopen` result and `name` is a nul-terminated literal.
    unsafe { libc::dlsym(handle as *mut c_void, name.as_ptr()) }
}

#[cfg(all(target_pointer_width = "64", unix))]
fn close_library(handle: usize) {
    // SAFETY: `handle` came from `dlopen` and is not used again.
    unsafe {
        libc::dlclose(handle as *mut c_void);
    }
}

#[cfg(all(target_pointer_width = "64", windows))]
fn open_library(name: &str) -> Option<usize> {
    let mut bytes = name.as_bytes().to_vec();
    bytes.push(0);
    // SAFETY: the name is a local nul-terminated buffer. A null module means the DLL is absent.
    let module = unsafe { windows_sys::Win32::System::LibraryLoader::LoadLibraryA(bytes.as_ptr()) };
    if module.is_null() {
        None
    } else {
        Some(module as usize)
    }
}

#[cfg(all(target_pointer_width = "64", windows))]
fn raw_symbol(handle: usize, name: &CStr) -> *mut c_void {
    // SAFETY: `handle` is a live module and `name` is a nul-terminated literal.
    let proc = unsafe {
        windows_sys::Win32::System::LibraryLoader::GetProcAddress(
            handle as *mut c_void,
            name.as_ptr().cast(),
        )
    };
    match proc {
        Some(fun) => {
            // SAFETY: the pointer bits are the export address. The caller transmutes them to the C ABI.
            unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, *mut c_void>(fun) }
        }
        None => ptr::null_mut(),
    }
}

#[cfg(all(target_pointer_width = "64", windows))]
fn close_library(handle: usize) {
    // SAFETY: `handle` came from `LoadLibraryA` and is not used again.
    unsafe {
        windows_sys::Win32::System::LibraryLoader::FreeLibrary(handle as *mut c_void);
    }
}
