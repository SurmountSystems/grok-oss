//! XLSX skill entry point. Workbook read, write, edit, formulas, and
//! formatting go through `office_oxide` in this process.
//!
//! The shell tool must route argv0 `grok-oss-xlsx` here. This file does not
//! start an interpreter. A command that would start one is not this entry.

use std::path::{Path, PathBuf};

use office_oxide::core::editable::EditablePackage;
use office_oxide::core::opc::PartName;
use office_oxide::core::relationships::rel_types;
use office_oxide::xlsx::edit::{CellValue as EditValue, EditableXlsx};
use office_oxide::xlsx::write::{CellData, CellStyle, XlsxWriter};
use office_oxide::xlsx::{CellRef, CellValue, XlsxDocument};
use serde::Deserialize;

const ENTRY_BIN: &str = "grok-oss-xlsx";
const MAX_FORMULA_CHARS: usize = 8192;

/// Host skill trees. A missing directory is not created.
pub const HOST_XLSX_SKILL_DIR: &str = "/home/hunter/.agents/skills/xlsx";
pub const BUNDLED_XLSX_SKILL_DIR: &str = "/home/hunter/.grok/bundled/skills/xlsx";

/// What `recalc.py` used to launch, before this entry existed.
pub const OLD_XLSX_PYTHON_SPAWN: &str = "python \"${SKILL_DIR}/scripts/recalc.py\" output.xlsx";

/// Cached error tokens the old recalc scan listed. This is not evaluation.
const EXCEL_ERRORS: &[&str] = &[
    "#VALUE!", "#DIV/0!", "#REF!", "#NAME?", "#NULL!", "#NUM!", "#N/A",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XlsxIntercept {
    pub command: String,
    pub op: XlsxOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XlsxOp {
    Read {
        path: String,
    },
    Write {
        path: String,
        json_path: String,
    },
    Edit {
        path: String,
        json_path: String,
        out: Option<String>,
    },
    Scan {
        path: String,
    },
    FromCsv {
        csv: String,
        xlsx: String,
        sheet: String,
    },
    Usage,
}

pub struct XlsxExec {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl XlsxIntercept {
    /// True when this command would start an interpreter. The parsed entry
    /// never does.
    pub fn spawns_python3(&self) -> bool {
        command_spawns_python3(&self.command)
    }

    /// True when argv0 is the Rust entry and the command does not start an
    /// interpreter.
    pub fn uses_rust_path(&self) -> bool {
        let argv0 = self.command.split_whitespace().next().unwrap_or("");
        file_name(argv0) == ENTRY_BIN && !self.spawns_python3()
    }
}

/// `true` when `command` would start python or python3.
pub fn command_spawns_python3(command: &str) -> bool {
    tokens(command).iter().any(|tok| is_python_interpreter(tok))
}

/// `Some` only for a clean `grok-oss-xlsx` command. Interpreter commands are
/// `None` so they are not this entry point.
pub fn try_parse_xlsx_intercept(command: &str) -> Option<XlsxIntercept> {
    let trimmed = command.trim();
    if trimmed.is_empty() || command_has_shell_glue(trimmed) {
        return None;
    }
    if command_spawns_python3(trimmed) {
        return None;
    }
    let toks = tokens(trimmed);
    let argv0 = toks.first()?;
    if file_name(argv0) != ENTRY_BIN {
        return None;
    }
    // `get` is None only when `toks` is empty. That miss stays `None`.
    let rest = toks.get(1..)?;
    let op = parse_op(rest)?;
    Some(XlsxIntercept {
        command: trimmed.to_string(),
        op,
    })
}

/// Run a parsed entry in this process. Does not start an interpreter.
pub fn execute_intercept(hit: &XlsxIntercept, cwd: &Path) -> XlsxExec {
    if hit.spawns_python3() || !hit.uses_rust_path() {
        return XlsxExec {
            stdout: String::new(),
            stderr: "the XLSX entry point does not spawn python3 and the work goes through the Rust path\n".to_string(),
            exit_code: 1,
        };
    }
    match run_op(&hit.op, cwd) {
        Ok(stdout) => XlsxExec {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        },
        Err(err) => XlsxExec {
            stdout: serde_json::json!({"error": err}).to_string() + "\n",
            stderr: format!("{err}\n"),
            exit_code: 1,
        },
    }
}

fn run_op(op: &XlsxOp, cwd: &Path) -> Result<String, String> {
    match op {
        XlsxOp::Usage => Ok(usage()),
        XlsxOp::Read { path } => {
            let doc = open_doc(&resolve(cwd, path))?;
            Ok(serde_json::to_string_pretty(&read_json(&doc)?).map_err(|e| e.to_string())? + "\n")
        }
        XlsxOp::Scan { path } => {
            let doc = open_doc(&resolve(cwd, path))?;
            Ok(serde_json::to_string_pretty(&scan_json(&doc)).map_err(|e| e.to_string())? + "\n")
        }
        XlsxOp::Write { path, json_path } => {
            let dest = resolve(cwd, path);
            let spec = read_spec::<WriteSpec>(&resolve(cwd, json_path))?;
            write_workbook(&dest, &spec)?;
            Ok(done_json(&dest)?)
        }
        XlsxOp::Edit {
            path,
            json_path,
            out,
        } => {
            let src = resolve(cwd, path);
            let dest = match out {
                Some(p) => resolve(cwd, p),
                None => src.clone(),
            };
            let spec = read_spec::<EditSpec>(&resolve(cwd, json_path))?;
            edit_workbook(&src, &dest, &spec)?;
            Ok(done_json(&dest)?)
        }
        XlsxOp::FromCsv { csv, xlsx, sheet } => {
            let dest = resolve(cwd, xlsx);
            write_csv(&resolve(cwd, csv), &dest, sheet)?;
            Ok(done_json(&dest)?)
        }
    }
}

fn usage() -> String {
    "\
grok-oss-xlsx read <workbook>
grok-oss-xlsx write <workbook> --json <spec>
grok-oss-xlsx edit <workbook> --json <spec> [--out <path>]
grok-oss-xlsx scan <workbook>
grok-oss-xlsx from-csv <table.csv> <workbook> [--sheet <name>]
"
    .to_string()
}

fn done_json(path: &Path) -> Result<String, String> {
    Ok(serde_json::json!({"status": "ok", "path": path.display().to_string()}).to_string() + "\n")
}

fn open_doc(path: &Path) -> Result<XlsxDocument, String> {
    if !path.is_file() {
        return Err(format!("file {} does not exist", path.display()));
    }
    XlsxDocument::open(path).map_err(|e| e.to_string())
}

fn read_spec<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_json(doc: &XlsxDocument) -> Result<serde_json::Value, String> {
    let mut sheets = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for sheet in &doc.worksheets {
        seen.insert(sheet.name.clone());
        let mut cells = Vec::new();
        for row in &sheet.rows {
            for cell in &row.cells {
                let value = json_cell_value(&cell.value, &doc.shared_strings);
                cells.push(serde_json::json!({
                    "ref": cell.reference.to_string(),
                    "value": value,
                    "formula": cell.formula(),
                }));
            }
        }
        sheets.push(serde_json::json!({
            "name": sheet.name,
            "cells": cells,
        }));
    }
    let unread: Vec<&str> = doc
        .workbook
        .sheets
        .iter()
        .filter(|s| !seen.contains(&s.name))
        .map(|s| s.name.as_str())
        .collect();
    Ok(serde_json::json!({
        "sheets": sheets,
        "unread_sheets": unread,
        "has_macros": doc.has_macros,
    }))
}

fn json_cell_value(
    value: &CellValue,
    sst: &office_oxide::xlsx::SharedStringTable,
) -> serde_json::Value {
    match value {
        CellValue::Empty => serde_json::Value::Null,
        CellValue::Number(n) => serde_json::json!(n),
        CellValue::String(s) => serde_json::Value::String(s.clone()),
        CellValue::SharedString(i) => sst
            .get(*i)
            .map(|s| serde_json::Value::String(s.to_string()))
            .unwrap_or(serde_json::Value::Null),
        CellValue::Boolean(b) => serde_json::Value::Bool(*b),
        CellValue::Error(e) => serde_json::Value::String(e.clone()),
        CellValue::Date(d) => serde_json::Value::String(d.to_string()),
    }
}

fn scan_json(doc: &XlsxDocument) -> serde_json::Value {
    let mut summary = serde_json::Map::new();
    let mut total_errors = 0usize;
    let mut total_formulas = 0usize;
    for sheet in &doc.worksheets {
        for row in &sheet.rows {
            for cell in &row.cells {
                if cell.formula().is_some() {
                    total_formulas += 1;
                }
                let text = match &cell.value {
                    CellValue::Error(e) => e.clone(),
                    CellValue::String(s) => s.clone(),
                    CellValue::SharedString(i) => {
                        doc.shared_strings.get(*i).unwrap_or("").to_string()
                    }
                    _ => String::new(),
                };
                if text.is_empty() {
                    continue;
                }
                for err in EXCEL_ERRORS {
                    if text.contains(err) {
                        total_errors += 1;
                        let slot = summary
                            .entry((*err).to_string())
                            .or_insert_with(|| serde_json::json!({"count": 0, "locations": []}));
                        if let Some(obj) = slot.as_object_mut() {
                            let count = obj.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                            obj.insert("count".into(), serde_json::json!(count + 1));
                            if let Some(locs) =
                                obj.get_mut("locations").and_then(|v| v.as_array_mut())
                            {
                                if locs.len() < 20 {
                                    locs.push(serde_json::Value::String(format!(
                                        "{}!{}",
                                        sheet.name, cell.reference
                                    )));
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    serde_json::json!({
        "status": if total_errors == 0 { "cached_clean" } else { "cached_errors" },
        "recalculated": false,
        "total_errors": total_errors,
        "total_formulas": total_formulas,
        "error_summary": summary,
        "note": "Cached cell text only. Formula evaluation is not performed.",
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteSpec {
    sheets: Vec<SheetSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetSpec {
    name: String,
    #[serde(default)]
    columns: Vec<ColSpec>,
    #[serde(default)]
    cells: Vec<CellSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColSpec {
    col: ColId,
    width: f64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ColId {
    Index(usize),
    Letters(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CellSpec {
    #[serde(rename = "ref")]
    cell_ref: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    bold: Option<bool>,
    #[serde(default)]
    italic: Option<bool>,
    #[serde(default)]
    font: Option<String>,
    #[serde(default)]
    font_color: Option<String>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    number_format: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditSpec {
    cells: Vec<EditCell>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditCell {
    #[serde(default)]
    sheet: Option<usize>,
    #[serde(default)]
    sheet_name: Option<String>,
    #[serde(rename = "ref")]
    cell_ref: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    bold: Option<bool>,
    #[serde(default)]
    italic: Option<bool>,
    #[serde(default)]
    font: Option<String>,
    #[serde(default)]
    font_color: Option<String>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    number_format: Option<String>,
}

fn write_workbook(dest: &Path, spec: &WriteSpec) -> Result<(), String> {
    if spec.sheets.is_empty() {
        return Err("write spec needs at least one sheet".into());
    }
    let mut names = std::collections::HashSet::new();
    for sheet in &spec.sheets {
        if sheet.name.trim().is_empty() {
            return Err("sheet name is empty".into());
        }
        if !names.insert(sheet.name.clone()) {
            return Err(format!("duplicate sheet name {}", sheet.name));
        }
    }
    let mut writer = XlsxWriter::new();
    for (idx, sheet) in spec.sheets.iter().enumerate() {
        writer.add_sheet(&sheet.name);
        for col in &sheet.columns {
            let n = col_index(&col.col)?;
            writer.sheet_set_column_width(idx, n, col.width);
        }
        for cell in &sheet.cells {
            let cref = CellRef::parse(&cell.cell_ref)
                .ok_or_else(|| format!("invalid cell reference {}", cell.cell_ref))?;
            let data = cell_data(cell)?;
            let style = cell_style(cell)?;
            let wrote = if style_is_set(cell) {
                writer.sheet_set_cell_styled(idx, cref.row as usize, cref.col as usize, data, style)
            } else {
                writer.sheet_set_cell(idx, cref.row as usize, cref.col as usize, data)
            };
            if !wrote {
                return Err(format!(
                    "cell {} was not written on sheet {}",
                    cell.cell_ref, sheet.name
                ));
            }
        }
    }
    writer.save(dest).map_err(|e| e.to_string())
}

fn style_is_set(cell: &CellSpec) -> bool {
    cell.bold == Some(true)
        || cell.italic == Some(true)
        || cell.font.as_ref().is_some_and(|s| !s.is_empty())
        || cell.font_color.is_some()
        || cell.fill.is_some()
        || cell.number_format.as_ref().is_some_and(|s| !s.is_empty())
}

fn cell_style(cell: &CellSpec) -> Result<CellStyle, String> {
    let mut style = CellStyle::new();
    if cell.bold == Some(true) {
        style.bold = true;
    }
    if cell.italic == Some(true) {
        style.italic = true;
    }
    if let Some(name) = cell.font.as_ref().filter(|s| !s.is_empty()) {
        style.font_name = Some(name.clone());
    }
    if let Some(color) = &cell.font_color {
        style.font_color = Some(normalize_hex(color)?);
    }
    if let Some(color) = &cell.fill {
        style.background_color = Some(normalize_hex(color)?);
    }
    if let Some(fmt) = cell.number_format.as_ref().filter(|s| !s.is_empty()) {
        style.number_format_code = Some(fmt.clone());
    }
    Ok(style)
}

fn cell_data(cell: &CellSpec) -> Result<CellData, String> {
    if let Some(formula) = &cell.formula {
        return Ok(CellData::Formula(normalize_formula(formula)?));
    }
    match &cell.value {
        None | Some(serde_json::Value::Null) => Ok(CellData::Empty),
        Some(serde_json::Value::Bool(b)) => Ok(CellData::Boolean(*b)),
        Some(serde_json::Value::Number(n)) => {
            let f = n
                .as_f64()
                .ok_or_else(|| format!("number {n} is not a float"))?;
            if !f.is_finite() {
                return Err("cell number must be finite".into());
            }
            Ok(CellData::Number(f))
        }
        Some(serde_json::Value::String(s)) => Ok(CellData::String(s.clone())),
        Some(other) => Err(format!(
            "cell value must be a string, number, bool, or null, not {other}"
        )),
    }
}

fn normalize_formula(formula: &str) -> Result<String, String> {
    let body = formula.trim().strip_prefix('=').unwrap_or(formula.trim());
    if body.is_empty() {
        return Err("formula is empty".into());
    }
    if body.chars().count() > MAX_FORMULA_CHARS {
        return Err(format!(
            "formula is longer than {MAX_FORMULA_CHARS} characters"
        ));
    }
    Ok(body.to_string())
}

fn normalize_hex(raw: &str) -> Result<String, String> {
    let s = raw.trim().trim_start_matches('#');
    let rgb = if s.len() == 8 && s[..2].eq_ignore_ascii_case("FF") {
        &s[2..]
    } else {
        s
    };
    if rgb.len() == 6 && rgb.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(rgb.to_ascii_uppercase())
    } else {
        Err(format!("color {raw} must be 6 hex digits"))
    }
}

fn col_index(col: &ColId) -> Result<usize, String> {
    match col {
        ColId::Index(n) => Ok(*n),
        ColId::Letters(s) => CellRef::parse_col(s)
            .map(|n| n as usize)
            .ok_or_else(|| format!("invalid column {s}")),
    }
}

fn edit_workbook(src: &Path, dest: &Path, spec: &EditSpec) -> Result<(), String> {
    if spec.cells.is_empty() {
        return Err("edit spec has no cells".into());
    }
    for cell in &spec.cells {
        if cell.bold.is_some()
            || cell.italic.is_some()
            || cell.font.is_some()
            || cell.font_color.is_some()
            || cell.fill.is_some()
            || cell.number_format.is_some()
        {
            return Err(
                "applying a new font, fill, or number format on an existing workbook is not ported; use write for a new workbook"
                    .into(),
            );
        }
        if cell.formula.is_none() && cell.value.is_none() {
            return Err(format!("cell {} has no value or formula", cell.cell_ref));
        }
    }
    let any_formula = spec.cells.iter().any(|c| c.formula.is_some());
    if any_formula {
        edit_via_package(src, dest, spec)
    } else {
        edit_values_only(src, dest, spec)
    }
}

fn edit_values_only(src: &Path, dest: &Path, spec: &EditSpec) -> Result<(), String> {
    let doc = open_doc(src)?;
    let mut book = EditableXlsx::open(src).map_err(|e| e.to_string())?;
    for cell in &spec.cells {
        let idx = sheet_index(&doc, cell.sheet, cell.sheet_name.as_deref())?;
        let cref = CellRef::parse(&cell.cell_ref)
            .ok_or_else(|| format!("invalid cell reference {}", cell.cell_ref))?;
        let value = edit_value(cell.value.as_ref())?;
        book.set_cell(idx, &cref.to_string(), value)
            .map_err(|e| e.to_string())?;
    }
    if dest == src {
        let tmp = sibling_part(dest);
        book.save(&tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    } else {
        book.save(dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn edit_value(value: Option<&serde_json::Value>) -> Result<EditValue, String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(EditValue::Empty),
        Some(serde_json::Value::Bool(b)) => Ok(EditValue::Boolean(*b)),
        Some(serde_json::Value::Number(n)) => {
            let f = n
                .as_f64()
                .ok_or_else(|| format!("number {n} is not a float"))?;
            if !f.is_finite() {
                return Err("cell number must be finite".into());
            }
            Ok(EditValue::Number(f))
        }
        Some(serde_json::Value::String(s)) => Ok(EditValue::String(s.clone())),
        Some(other) => Err(format!(
            "cell value must be a string, number, bool, or null, not {other}"
        )),
    }
}

fn edit_via_package(src: &Path, dest: &Path, spec: &EditSpec) -> Result<(), String> {
    let doc = open_doc(src)?;
    let mut pkg = EditablePackage::open(src).map_err(|e| e.to_string())?;
    for cell in &spec.cells {
        let idx = sheet_index(&doc, cell.sheet, cell.sheet_name.as_deref())?;
        let part = worksheet_part(&pkg, idx)?;
        let bytes = pkg
            .get_part(&part)
            .ok_or_else(|| format!("missing worksheet part {}", part.as_str()))?
            .to_vec();
        let xml = String::from_utf8(bytes).map_err(|e| e.to_string())?;
        let xml_value = if let Some(formula) = &cell.formula {
            XmlCell::Formula(normalize_formula(formula)?)
        } else {
            xml_cell_from_json(cell.value.as_ref())?
        };
        let cref = CellRef::parse(&cell.cell_ref)
            .ok_or_else(|| format!("invalid cell reference {}", cell.cell_ref))?;
        let new_xml = set_cell_in_xml(&xml, &cref.to_string(), &xml_value)?;
        pkg.set_part(part, new_xml.into_bytes());
    }
    save_package(&pkg, dest)
}

fn xml_cell_from_json(value: Option<&serde_json::Value>) -> Result<XmlCell, String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(XmlCell::Empty),
        Some(serde_json::Value::Bool(b)) => Ok(XmlCell::Bool(*b)),
        Some(serde_json::Value::Number(n)) => {
            let f = n
                .as_f64()
                .ok_or_else(|| format!("number {n} is not a float"))?;
            if !f.is_finite() {
                return Err("cell number must be finite".into());
            }
            Ok(XmlCell::Number(f))
        }
        Some(serde_json::Value::String(s)) => Ok(XmlCell::String(s.clone())),
        Some(other) => Err(format!(
            "cell value must be a string, number, bool, or null, not {other}"
        )),
    }
}

fn sheet_index(
    doc: &XlsxDocument,
    sheet: Option<usize>,
    sheet_name: Option<&str>,
) -> Result<usize, String> {
    if let Some(name) = sheet_name {
        return doc
            .workbook
            .sheets
            .iter()
            .position(|s| s.name == name)
            .ok_or_else(|| format!("no sheet named {name}"));
    }
    let idx = sheet.unwrap_or(0);
    if idx >= doc.workbook.sheets.len() {
        return Err(format!("sheet index {idx} is out of range"));
    }
    Ok(idx)
}

fn workbook_part(pkg: &EditablePackage) -> Result<PartName, String> {
    let rels = pkg.package_rels();
    if let Some(rel) = rels.first_by_type(rel_types::OFFICE_DOCUMENT) {
        rels.resolve_target_from_root(&rel.id)
            .map_err(|e| e.to_string())
    } else {
        PartName::new("/xl/workbook.xml").map_err(|e| e.to_string())
    }
}

fn worksheet_part(pkg: &EditablePackage, sheet_index: usize) -> Result<PartName, String> {
    let workbook = workbook_part(pkg)?;
    let wb_xml = pkg
        .get_part(&workbook)
        .ok_or_else(|| "missing workbook".to_string())?
        .to_vec();
    let info = office_oxide::xlsx::WorkbookInfo::parse(&wb_xml).map_err(|e| e.to_string())?;
    let sheet = info
        .sheets
        .get(sheet_index)
        .ok_or_else(|| format!("no sheet at index {sheet_index}"))?;
    let rels = pkg
        .part_rels(&workbook)
        .ok_or_else(|| format!("workbook {} has no relationships", workbook.as_str()))?;
    let rel = rels
        .get_by_id(&sheet.rel_id)
        .ok_or_else(|| format!("sheet {} has no relationship", sheet.name))?;
    if rel.rel_type != rel_types::WORKSHEET {
        return Err(format!(
            "sheet {} is not a worksheet; chart sheets are not ported",
            sheet.name
        ));
    }
    rels.resolve_target(&sheet.rel_id, &workbook)
        .map_err(|e| e.to_string())
}

fn save_package(pkg: &EditablePackage, dest: &Path) -> Result<(), String> {
    let tmp = sibling_part(dest);
    pkg.save(&tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn sibling_part(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("workbook.xlsx");
    let parent = dest.parent().filter(|p| !p.as_os_str().is_empty());
    match parent {
        Some(dir) => dir.join(format!(".{name}.part")),
        None => PathBuf::from(format!(".{name}.part")),
    }
}

fn write_csv(csv_path: &Path, dest: &Path, sheet_name: &str) -> Result<(), String> {
    if sheet_name.trim().is_empty() {
        return Err("sheet name is empty".into());
    }
    let text =
        std::fs::read_to_string(csv_path).map_err(|e| format!("{}: {e}", csv_path.display()))?;
    let delim = delimiter_for(csv_path);
    let rows = parse_delimited(&text, delim)?;
    if rows.is_empty() {
        return Err("table has no rows".into());
    }
    let mut writer = XlsxWriter::new();
    writer.add_sheet(sheet_name);
    for (r, row) in rows.iter().enumerate() {
        for (c, field) in row.iter().enumerate() {
            let data = if r > 0 {
                csv_cell(field)
            } else {
                CellData::String(field.clone())
            };
            let style = CellStyle::new().font_name("Arial");
            let style = if r == 0 { style.bold() } else { style };
            if !writer.sheet_set_cell_styled(0, r, c, data, style) {
                return Err(format!("csv cell row {r} col {c} was not written"));
            }
        }
    }
    writer.save(dest).map_err(|e| e.to_string())
}

fn delimiter_for(path: &Path) -> u8 {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "tsv" | "tab" => b'\t',
        _ => b',',
    }
}

fn csv_cell(field: &str) -> CellData {
    if field.is_empty() {
        return CellData::Empty;
    }
    if let Some(n) = plain_number(field) {
        CellData::Number(n)
    } else {
        CellData::String(field.to_string())
    }
}

/// A plain integer or decimal. Leading zeros stay text (`00123`).
fn plain_number(field: &str) -> Option<f64> {
    let bytes = field.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let rest = field.strip_prefix('-').unwrap_or(field);
    if rest.starts_with('0') && rest.len() > 1 && !rest.starts_with("0.") {
        return None;
    }
    if !rest.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    if rest.bytes().filter(|b| *b == b'.').count() > 1 {
        return None;
    }
    let n: f64 = field.parse().ok()?;
    n.is_finite().then_some(n)
}

fn parse_delimited(text: &str, delim: u8) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        if c == '"' && field.is_empty() {
            in_quotes = true;
            continue;
        }
        if c == delim as char {
            row.push(std::mem::take(&mut field));
            continue;
        }
        if c == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
            continue;
        }
        if c == '\r' {
            continue;
        }
        field.push(c);
    }
    if in_quotes {
        return Err("table has an unterminated quote".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

enum XmlCell {
    Empty,
    String(String),
    Number(f64),
    Bool(bool),
    Formula(String),
}

fn set_cell_in_xml(xml: &str, cell_ref: &str, value: &XmlCell) -> Result<String, String> {
    let (row, col) = split_ref(cell_ref)?;
    if let Some(replaced) = replace_existing_cell(xml, cell_ref, value) {
        return Ok(replaced);
    }
    let cell_xml = render_cell(cell_ref, "", value);
    match locate_row(xml, row) {
        Some(RowShape::SelfClosing {
            open_start,
            open_end,
        }) => {
            let open_tag = &xml[open_start..open_end];
            let reopened = format!("{}>{cell_xml}</row>", &open_tag[..open_tag.len() - 2]);
            let mut result = String::with_capacity(xml.len() + reopened.len());
            result.push_str(&xml[..open_start]);
            result.push_str(&reopened);
            result.push_str(&xml[open_end..]);
            return Ok(result);
        }
        Some(RowShape::Paired {
            body_start,
            body_end,
        }) => {
            let at = body_start + cell_insert_offset(&xml[body_start..body_end], col);
            let mut result = String::with_capacity(xml.len() + cell_xml.len());
            result.push_str(&xml[..at]);
            result.push_str(&cell_xml);
            result.push_str(&xml[at..]);
            return Ok(result);
        }
        None => {}
    }
    let row_xml = format!(r#"<row r="{row}">{cell_xml}</row>"#);
    if let Some(at) = xml.find("<sheetData") {
        if let Some(rel_close) = xml[at..].find('>') {
            let close = at + rel_close;
            if xml.as_bytes().get(close.wrapping_sub(1)) == Some(&b'/') {
                let mut result = String::with_capacity(xml.len() + row_xml.len() + 16);
                result.push_str(&xml[..close - 1]);
                result.push('>');
                result.push_str(&row_xml);
                result.push_str("</sheetData>");
                result.push_str(&xml[close + 1..]);
                return Ok(result);
            }
        }
    }
    if let Some(sd_end) = xml.find("</sheetData>") {
        let sd_body_start = xml
            .find("<sheetData")
            .and_then(|i| xml[i..].find('>').map(|j| i + j + 1))
            .unwrap_or(sd_end);
        let at = row_insert_offset(xml, sd_body_start, sd_end, row);
        let mut result = String::with_capacity(xml.len() + row_xml.len());
        result.push_str(&xml[..at]);
        result.push_str(&row_xml);
        result.push_str(&xml[at..]);
        return Ok(result);
    }
    Err("worksheet has no sheetData to write a cell into".into())
}

fn split_ref(cell_ref: &str) -> Result<(u32, &str), String> {
    let col_end = cell_ref
        .bytes()
        .position(|b| b.is_ascii_digit())
        .filter(|p| *p > 0)
        .ok_or_else(|| format!("invalid cell reference {cell_ref}"))?;
    let col = &cell_ref[..col_end];
    let row: u32 = cell_ref[col_end..]
        .parse()
        .map_err(|_| format!("invalid cell reference {cell_ref}"))?;
    if row == 0 {
        return Err(format!("invalid cell reference {cell_ref}"));
    }
    Ok((row, col))
}

fn replace_existing_cell(xml: &str, cell_ref: &str, value: &XmlCell) -> Option<String> {
    let needle = format!(r#"<c r="{cell_ref}""#);
    let start = xml.find(&needle)?;
    let rest = &xml[start..];
    let tag_end = rest.find('>')?;
    let open_tag = &rest[..=tag_end];
    let end = if open_tag.ends_with("/>") {
        start + tag_end + 1
    } else {
        start + rest.find("</c>")? + 4
    };
    let attrs = preserved_attrs(open_tag);
    let cell_xml = render_cell(cell_ref, &attrs, value);
    let mut result = String::with_capacity(xml.len());
    result.push_str(&xml[..start]);
    result.push_str(&cell_xml);
    result.push_str(&xml[end..]);
    Some(result)
}

fn preserved_attrs(open_tag: &str) -> String {
    let body = open_tag
        .trim_start_matches("<c")
        .trim_end_matches('>')
        .trim_end_matches('/');
    let mut out = String::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let name_start = i;
        while bytes
            .get(i)
            .is_some_and(|b| *b != b'=' && !b.is_ascii_whitespace())
        {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name = &body[name_start..i];
        while bytes
            .get(i)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'=')
        {
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            break;
        }
        i += 1;
        let value_start = i;
        while bytes.get(i).is_some_and(|b| *b != b'"') {
            i += 1;
        }
        let value = &body[value_start..i.min(body.len())];
        if i < bytes.len() {
            i += 1;
        }
        if !name.is_empty() && name != "r" && name != "t" {
            out.push_str(&format!(r#" {name}="{value}""#));
        }
    }
    out
}

fn render_cell(cell_ref: &str, attrs: &str, value: &XmlCell) -> String {
    match value {
        XmlCell::Empty => format!(r#"<c r="{cell_ref}"{attrs}/>"#),
        XmlCell::String(s) => {
            let escaped = escape_xml(s);
            let space = if escaped.trim() != escaped {
                r#" xml:space="preserve""#
            } else {
                ""
            };
            format!(
                r#"<c r="{cell_ref}"{attrs} t="inlineStr"><is><t{space}>{escaped}</t></is></c>"#
            )
        }
        XmlCell::Number(n) => format!(r#"<c r="{cell_ref}"{attrs}><v>{n}</v></c>"#),
        XmlCell::Bool(b) => {
            let v = if *b { "1" } else { "0" };
            format!(r#"<c r="{cell_ref}"{attrs} t="b"><v>{v}</v></c>"#)
        }
        XmlCell::Formula(f) => {
            let escaped = escape_xml(f);
            format!(r#"<c r="{cell_ref}"{attrs}><f>{escaped}</f></c>"#)
        }
    }
}

fn escape_xml(s: &str) -> String {
    office_oxide::core::xml::sanitize_xml_text(s)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

enum RowShape {
    SelfClosing { open_start: usize, open_end: usize },
    Paired { body_start: usize, body_end: usize },
}

fn locate_row(xml: &str, row: u32) -> Option<RowShape> {
    let needle = format!(r#"<row r="{row}""#);
    let row_start = xml.find(&needle)?;
    let rest = &xml[row_start..];
    let tag_end = rest.find('>')?;
    let open_tag = &rest[..=tag_end];
    if open_tag.ends_with("/>") {
        return Some(RowShape::SelfClosing {
            open_start: row_start,
            open_end: row_start + tag_end + 1,
        });
    }
    let body_start = row_start + tag_end + 1;
    let body_end = body_start + xml[body_start..].find("</row>")?;
    Some(RowShape::Paired {
        body_start,
        body_end,
    })
}

fn col_index_letters(letters: &str) -> u32 {
    letters.bytes().fold(0u32, |acc, b| {
        acc.saturating_mul(26)
            .saturating_add(u32::from(b.to_ascii_uppercase() - b'A') + 1)
    })
}

fn cell_insert_offset(row_body: &str, col: &str) -> usize {
    let col = col_index_letters(col);
    let mut search = 0usize;
    while let Some(rel) = row_body[search..].find("<c r=\"") {
        let at = search + rel;
        let ref_start = at + 6;
        let Some(len) = row_body[ref_start..].find('"') else {
            break;
        };
        let existing = &row_body[ref_start..ref_start + len];
        match split_ref(existing) {
            Ok((_, existing_col)) if col_index_letters(existing_col) > col => return at,
            _ => {}
        }
        search = ref_start + len;
    }
    row_body.len()
}

fn row_insert_offset(xml: &str, sd_body_start: usize, sd_end: usize, row: u32) -> usize {
    let body = &xml[sd_body_start..sd_end];
    let mut search = 0usize;
    while let Some(rel) = body[search..].find("<row r=\"") {
        let at = search + rel;
        let num_start = at + 8;
        let Some(len) = body[num_start..].find('"') else {
            break;
        };
        if let Ok(existing) = body[num_start..num_start + len].parse::<u32>() {
            if existing > row {
                return sd_body_start + at;
            }
        }
        search = num_start + len;
    }
    sd_end
}

fn parse_op(args: &[String]) -> Option<XlsxOp> {
    let Some(verb) = args.first().map(String::as_str) else {
        return Some(XlsxOp::Usage);
    };
    match verb {
        "read" if args.len() == 2 => Some(XlsxOp::Read {
            path: args.get(1).cloned()?,
        }),
        "scan" if args.len() == 2 => Some(XlsxOp::Scan {
            path: args.get(1).cloned()?,
        }),
        "write" => parse_write(args),
        "edit" => parse_edit(args),
        "from-csv" => parse_from_csv(args),
        _ => None,
    }
}

fn parse_write(args: &[String]) -> Option<XlsxOp> {
    if args.len() != 4 || args.get(2).map(String::as_str) != Some("--json") {
        return None;
    }
    Some(XlsxOp::Write {
        path: args.get(1).cloned()?,
        json_path: args.get(3).cloned()?,
    })
}

fn parse_edit(args: &[String]) -> Option<XlsxOp> {
    if args.len() == 4 && args.get(2).map(String::as_str) == Some("--json") {
        return Some(XlsxOp::Edit {
            path: args.get(1).cloned()?,
            json_path: args.get(3).cloned()?,
            out: None,
        });
    }
    if args.len() == 6
        && args.get(2).map(String::as_str) == Some("--json")
        && args.get(4).map(String::as_str) == Some("--out")
    {
        return Some(XlsxOp::Edit {
            path: args.get(1).cloned()?,
            json_path: args.get(3).cloned()?,
            out: Some(args.get(5).cloned()?),
        });
    }
    None
}

fn parse_from_csv(args: &[String]) -> Option<XlsxOp> {
    if args.len() == 3 {
        return Some(XlsxOp::FromCsv {
            csv: args.get(1).cloned()?,
            xlsx: args.get(2).cloned()?,
            sheet: "Sheet".to_string(),
        });
    }
    if args.len() == 5
        && args.get(3).map(String::as_str) == Some("--sheet")
        && args.get(4).is_some_and(|sheet| !sheet.is_empty())
    {
        return Some(XlsxOp::FromCsv {
            csv: args.get(1).cloned()?,
            xlsx: args.get(2).cloned()?,
            sheet: args.get(4).cloned()?,
        });
    }
    None
}

fn command_has_shell_glue(command: &str) -> bool {
    command.contains("&&")
        || command.contains("||")
        || command.contains(';')
        || command.contains('|')
        || command.contains('\n')
        || command.contains('`')
        || command.contains("$(")
}

fn is_python_interpreter(tok: &str) -> bool {
    let base = file_name(tok);
    if matches!(base, "python" | "python2" | "python3") {
        return true;
    }
    let Some(rest) = base.strip_prefix("python3.") else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

fn file_name(tok: &str) -> &str {
    Path::new(tok)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(tok)
}

fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\r' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '\'' => {
                for c2 in chars.by_ref() {
                    if c2 == '\'' {
                        break;
                    }
                    cur.push(c2);
                }
            }
            '"' => {
                while let Some(c2) = chars.next() {
                    if c2 == '"' {
                        break;
                    }
                    if c2 == '\\' {
                        if let Some(n) = chars.next() {
                            cur.push(n);
                        }
                    } else {
                        cur.push(c2);
                    }
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn resolve(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

/// `true` when the skill tree still has a Python file or tells the reader to
/// start an interpreter. A missing directory is not a spawn.
pub fn skill_tree_spawns_python3(root: &Path) -> bool {
    if !root.is_dir() {
        return false;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            let ft = ent.file_type().ok();
            let is_symlink = ft.as_ref().is_some_and(|t| t.is_symlink());
            if is_symlink {
                continue;
            }
            if ft.as_ref().is_some_and(|t| t.is_dir()) {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("py") {
                return true;
            }
            if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("sh") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if prose_spawns_python3(&text) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn prose_spawns_python3(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("python3")
        || lower.contains("python ")
        || lower.contains("python\"")
        || lower.contains("openpyxl")
        || lower.contains("pandas")
        || lower.contains("xlsxwriter")
        || lower.contains("xlrd")
}

#[cfg(test)]
mod tests {
    use super::*;
    use office_oxide::core::theme::RgbColor;
    use office_oxide::xlsx::CellValue;

    #[test]
    fn xlsx_entry_point_does_not_spawn_python3() {
        let old = OLD_XLSX_PYTHON_SPAWN;
        assert!(
            command_spawns_python3(old) && try_parse_xlsx_intercept(old).is_none(),
            "the XLSX entry point does not spawn python3 and the work goes through the Rust path"
        );
        let cmd = "grok-oss-xlsx read workbook.xlsx";
        let hit = try_parse_xlsx_intercept(cmd).expect("rust entry");
        assert!(
            hit.uses_rust_path() && !hit.spawns_python3(),
            "the XLSX entry point does not spawn python3 and the work goes through the Rust path"
        );
        for root in [HOST_XLSX_SKILL_DIR, BUNDLED_XLSX_SKILL_DIR] {
            let path = Path::new(root);
            if !path.is_dir() {
                continue;
            }
            assert!(
                !skill_tree_spawns_python3(path),
                "the XLSX entry point does not spawn python3 and the work goes through the Rust path"
            );
        }
        let trip = round_trip_formula_format_and_edit();
        assert!(
            trip.is_ok(),
            "the XLSX entry point does not spawn python3 and the work goes through the Rust path ({trip:?})"
        );
    }

    #[test]
    fn formula_edit_keeps_style_attribute() {
        let xml =
            r#"<sheetData><row r="1"><c r="A1" s="5"><v>1</v></c><c r="C1"/></row></sheetData>"#;
        let got = set_cell_in_xml(xml, "A1", &XmlCell::Formula("SUM(B1)".into())).unwrap();
        assert!(
            got.contains(r#"<c r="A1" s="5"><f>SUM(B1)</f></c>"#),
            "{got}"
        );
        assert!(got.contains(r#"<c r="C1"/>"#), "{got}");
    }

    fn round_trip_formula_format_and_edit() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let book = dir.path().join("model.xlsx");
        let spec_path = dir.path().join("spec.json");
        let spec = r#"{
            "sheets": [{
                "name": "Model",
                "columns": [{"col": "A", "width": 20}],
                "cells": [
                    {"ref": "A1", "value": 2, "bold": true, "font": "Arial", "font_color": "0000FF", "fill": "FFFF00"},
                    {"ref": "A2", "value": 3, "font": "Arial"},
                    {"ref": "B1", "formula": "=SUM(A1:A2)", "number_format": "0.0%", "font": "Arial"}
                ]
            }]
        }"#;
        std::fs::write(&spec_path, spec).map_err(|e| e.to_string())?;
        let cwd = dir.path();
        let write_cmd = format!(
            "grok-oss-xlsx write {} --json {}",
            book.display(),
            spec_path.display()
        );
        let hit = try_parse_xlsx_intercept(&write_cmd).ok_or("write did not parse")?;
        let exec = execute_intercept(&hit, cwd);
        if exec.exit_code != 0 {
            return Err(format!("write failed: {} {}", exec.stdout, exec.stderr));
        }
        assert_written(&book)?;
        let edit_path = dir.path().join("edit.json");
        std::fs::write(
            &edit_path,
            r#"{"cells":[{"sheet_name":"Model","ref":"A1","value":5}]}"#,
        )
        .map_err(|e| e.to_string())?;
        let edit_cmd = format!(
            "grok-oss-xlsx edit {} --json {}",
            book.display(),
            edit_path.display()
        );
        let hit = try_parse_xlsx_intercept(&edit_cmd).ok_or("edit did not parse")?;
        let exec = execute_intercept(&hit, cwd);
        if exec.exit_code != 0 {
            return Err(format!(
                "value edit failed: {} {}",
                exec.stdout, exec.stderr
            ));
        }
        let formula_path = dir.path().join("formula.json");
        std::fs::write(
            &formula_path,
            r#"{"cells":[{"ref":"B1","formula":"SUM(A2:A2)"}]}"#,
        )
        .map_err(|e| e.to_string())?;
        let before = style_index_of(&book, "B1")?;
        let formula_cmd = format!(
            "grok-oss-xlsx edit {} --json {}",
            book.display(),
            formula_path.display()
        );
        let hit = try_parse_xlsx_intercept(&formula_cmd).ok_or("formula edit did not parse")?;
        let exec = execute_intercept(&hit, cwd);
        if exec.exit_code != 0 {
            return Err(format!(
                "formula edit failed: {} {}",
                exec.stdout, exec.stderr
            ));
        }
        let mut doc = XlsxDocument::open(&book).map_err(|e| e.to_string())?;
        doc.ensure_styles();
        let sheet = doc
            .worksheets
            .iter()
            .find(|s| s.name == "Model")
            .ok_or("missing Model")?;
        let a1 = find_cell(sheet, "A1")?;
        match &a1.value {
            CellValue::Number(n) if (*n - 5.0).abs() < 1e-9 => {}
            other => return Err(format!("A1 value {other:?}")),
        }
        let b1 = find_cell(sheet, "B1")?;
        if b1.formula() != Some("SUM(A2:A2)") {
            return Err(format!("B1 formula {:?}", b1.formula()));
        }
        if b1.style_index != before {
            return Err(format!(
                "B1 style {:?} changed from {before:?}",
                b1.style_index
            ));
        }
        assert_a1_format(&doc)?;
        let scan = try_parse_xlsx_intercept(&format!("grok-oss-xlsx scan {}", book.display()))
            .ok_or("scan did not parse")?;
        let exec = execute_intercept(&scan, cwd);
        if exec.exit_code != 0 || !exec.stdout.contains("\"recalculated\": false") {
            return Err(format!("scan {}", exec.stdout));
        }
        if exec.stdout.contains("python") {
            return Err("scan output named an interpreter".into());
        }
        Ok(())
    }

    fn assert_written(path: &Path) -> Result<(), String> {
        let mut doc = XlsxDocument::open(path).map_err(|e| e.to_string())?;
        doc.ensure_styles();
        let sheet = doc
            .worksheets
            .iter()
            .find(|s| s.name == "Model")
            .ok_or("missing Model")?;
        let a1 = find_cell(sheet, "A1")?;
        match &a1.value {
            CellValue::Number(n) if (*n - 2.0).abs() < 1e-9 => {}
            other => return Err(format!("A1 before edit {other:?}")),
        }
        let b1 = find_cell(sheet, "B1")?;
        if b1.formula() != Some("SUM(A1:A2)") {
            return Err(format!("wrote formula {:?}", b1.formula()));
        }
        assert_a1_format(&doc)?;
        let xml = first_sheet_xml(path)?;
        if !xml.contains("width=\"20.00\"") {
            return Err(format!("column width missing in {xml}"));
        }
        if !xml.contains("<f>SUM(A1:A2)</f>") {
            return Err("formula element missing".into());
        }
        Ok(())
    }

    fn assert_a1_format(doc: &XlsxDocument) -> Result<(), String> {
        let styles = doc.styles.as_ref().ok_or("styles missing")?;
        let sheet = doc
            .worksheets
            .iter()
            .find(|s| s.name == "Model")
            .ok_or("missing Model")?;
        let a1 = find_cell(sheet, "A1")?;
        let idx = a1.style_index.ok_or("A1 has no style")?;
        let fmt = styles
            .cell_formats
            .get(idx as usize)
            .ok_or("style index missing")?;
        let font = fmt
            .font_index
            .and_then(|i| styles.fonts.get(i as usize))
            .ok_or("A1 font missing")?;
        if font.name.as_deref() != Some("Arial") || !font.bold {
            return Err(format!("A1 font {:?}", font.name));
        }
        let rgb = font
            .color
            .as_ref()
            .and_then(|c| c.resolve_opt(None))
            .ok_or("A1 font color missing")?;
        if rgb != RgbColor([0, 0, 255]) {
            return Err(format!("A1 font color {rgb:?}"));
        }
        let fill = fmt
            .fill_index
            .and_then(|i| styles.fills.get(i as usize))
            .ok_or("A1 fill missing")?;
        let fill_rgb = fill
            .solid_color()
            .and_then(|c| c.resolve_opt(None))
            .ok_or("A1 fill color missing")?;
        if fill_rgb != RgbColor([255, 255, 0]) {
            return Err(format!("A1 fill {fill_rgb:?}"));
        }
        if !styles.number_formats.values().any(|c| c == "0.0%") {
            return Err("number format 0.0% missing".into());
        }
        Ok(())
    }

    fn style_index_of(path: &Path, cell_ref: &str) -> Result<Option<u32>, String> {
        let doc = XlsxDocument::open(path).map_err(|e| e.to_string())?;
        let sheet = doc
            .worksheets
            .iter()
            .find(|s| s.name == "Model")
            .ok_or("missing Model")?;
        Ok(find_cell(sheet, cell_ref)?.style_index)
    }

    fn find_cell<'a>(
        sheet: &'a office_oxide::xlsx::Worksheet,
        cell_ref: &str,
    ) -> Result<&'a office_oxide::xlsx::Cell, String> {
        for row in &sheet.rows {
            for cell in &row.cells {
                if cell.reference.to_string() == cell_ref {
                    return Ok(cell);
                }
            }
        }
        Err(format!("missing {cell_ref}"))
    }

    fn first_sheet_xml(path: &Path) -> Result<String, String> {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?.to_string();
            if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                let mut xml = String::new();
                entry.read_to_string(&mut xml).map_err(|e| e.to_string())?;
                return Ok(xml);
            }
        }
        Err("no worksheet part".into())
    }
}
