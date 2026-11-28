//! Fillable-field inventory and value writing. Same field JSON shape as the old helper.

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde_json::{Map, Value};

use crate::pdf_obj::{
    catalog_id, err, kid_ids, load, obj_text, owned_dict, owned_dict_at, page_size, pdf_string,
    save, usage, PdfResult,
};

struct FieldNode {
    id: ObjectId,
    qualified: String,
    ft: String,
    kids: Vec<ObjectId>,
    dict: Dictionary,
}

pub fn check_fillable_main() -> PdfResult<()> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        usage("Usage: check_fillable_fields [input pdf]");
    };
    let doc = load(&path)?;
    if acro_field_roots(&doc)?.is_empty() {
        println!(
            "This PDF does not have fillable form fields; you will need to visually determine where to enter data"
        );
    } else {
        let nodes = collect_fields(&doc)?;
        if nodes.is_empty() {
            println!(
                "This PDF does not have fillable form fields; you will need to visually determine where to enter data"
            );
        } else {
            println!("This PDF has fillable form fields");
        }
    }
    Ok(())
}

pub fn extract_fields_main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        usage("Usage: extract_form_field_info [input pdf] [output json]");
    }
    let doc = load(&args[0])?;
    let fields = field_info_json(&doc)?;
    let body = serde_json::to_string_pretty(&fields).map_err(err)?;
    std::fs::write(&args[1], format!("{body}\n")).map_err(err)?;
    let count = fields.as_array().map(|a| a.len()).unwrap_or(0);
    println!("Wrote {count} fields to {}", args[1]);
    Ok(())
}

pub fn fill_fields_main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        usage("Usage: fill_fillable_fields [input pdf] [field_values.json] [output pdf]");
    }
    let raw = std::fs::read_to_string(&args[1]).map_err(err)?;
    let values: Vec<Value> = serde_json::from_str(&raw).map_err(err)?;
    let mut doc = load(&args[0])?;
    let info = field_info_json(&doc)?;
    let known = info.as_array().cloned().unwrap_or_default();
    let mut has_error = false;
    for field in &values {
        let field_id = field.get("field_id").and_then(|v| v.as_str()).unwrap_or("");
        let page = field.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
        let existing = known
            .iter()
            .find(|item| item.get("field_id").and_then(|v| v.as_str()) == Some(field_id));
        let Some(existing) = existing else {
            has_error = true;
            println!("ERROR: `{field_id}` is not a valid field ID");
            continue;
        };
        let expected = existing.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
        if page != expected {
            has_error = true;
            println!(
                "ERROR: Incorrect page number for `{field_id}` (got {page}, expected {expected})"
            );
            continue;
        }
        if let Some(value) = field.get("value").and_then(|v| v.as_str()) {
            if let Some(message) = validation_error(existing, value) {
                println!("{message}");
                has_error = true;
            }
        }
    }
    if has_error {
        std::process::exit(1);
    }
    apply_field_values(&mut doc, &values)?;
    set_need_appearances(&mut doc)?;
    save(&mut doc, &args[2])?;
    Ok(())
}

fn validation_error(field_info: &Value, field_value: &str) -> Option<String> {
    let field_type = field_info
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let field_id = field_info
        .get("field_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match field_type {
        "checkbox" => {
            let checked = field_info
                .get("checked_value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let unchecked = field_info
                .get("unchecked_value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if field_value != checked && field_value != unchecked {
                return Some(format!(
                    "ERROR: Invalid value \"{field_value}\" for checkbox field \"{field_id}\". The checked value is \"{checked}\" and the unchecked value is \"{unchecked}\""
                ));
            }
        }
        "radio_group" => {
            let options = option_values(field_info.get("radio_options"));
            if !options.iter().any(|opt| opt == field_value) {
                return Some(format!(
                    "ERROR: Invalid value \"{field_value}\" for radio group field \"{field_id}\". Valid values are: {options:?}"
                ));
            }
        }
        "choice" => {
            let options = option_values(field_info.get("choice_options"));
            if !options.iter().any(|opt| opt == field_value) {
                return Some(format!(
                    "ERROR: Invalid value \"{field_value}\" for choice field \"{field_id}\". Valid values are: {options:?}"
                ));
            }
        }
        _ => {}
    }
    None
}

fn option_values(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("value")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn field_info_json(doc: &Document) -> PdfResult<Value> {
    let nodes = collect_fields(doc)?;
    let mut leaves = Vec::new();
    let mut radios = Vec::new();
    for node in &nodes {
        if !node.kids.is_empty() && node.ft == "/Btn" {
            radios.push(node);
        } else if node.kids.is_empty() {
            leaves.push(node);
        }
    }
    let mut page_of: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    let mut radio_options: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (number, page_id) in doc.get_pages() {
        let annots = doc.get_page_annotations(page_id).unwrap_or_default();
        for annot in annots {
            let qualified = full_name_from_dict(doc, annot);
            if qualified.is_empty() {
                continue;
            }
            page_of
                .entry(qualified.clone())
                .or_insert(i64::from(number));
            if radios.iter().any(|radio| radio.qualified == qualified) {
                for state in appearance_states(doc, annot) {
                    if state != "/Off" {
                        radio_options
                            .entry(qualified.clone())
                            .or_default()
                            .push(state);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    for node in leaves {
        let stripped = strip_xfa_prefix(&node.qualified);
        let Some(page) = page_of.get(&node.qualified).copied() else {
            println!("Unable to determine location for field id: {stripped}, ignoring");
            continue;
        };
        out.push(leaf_json(doc, node, &stripped, page));
    }
    for node in radios {
        let Some(page) = page_of.get(&node.qualified).copied() else {
            println!(
                "Unable to determine location for field id: {}, ignoring",
                node.qualified
            );
            continue;
        };
        let mut options = radio_options.remove(&node.qualified).unwrap_or_default();
        options.sort();
        options.dedup();
        let mut map = Map::new();
        map.insert("field_id".into(), Value::String(node.qualified.clone()));
        map.insert("type".into(), Value::String("radio_group".into()));
        map.insert("page".into(), Value::from(page));
        map.insert(
            "radio_options".into(),
            Value::Array(
                options
                    .into_iter()
                    .map(|value| {
                        let mut opt = Map::new();
                        opt.insert("value".into(), Value::String(value));
                        Value::Object(opt)
                    })
                    .collect(),
            ),
        );
        out.push(Value::Object(map));
    }
    out.sort_by(|left, right| {
        let page_left = left.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
        let page_right = right.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
        let id_left = left.get("field_id").and_then(|v| v.as_str()).unwrap_or("");
        let id_right = right.get("field_id").and_then(|v| v.as_str()).unwrap_or("");
        page_left.cmp(&page_right).then(id_left.cmp(id_right))
    });
    Ok(Value::Array(out))
}

fn leaf_json(doc: &Document, node: &FieldNode, field_id: &str, page: i64) -> Value {
    let mut map = Map::new();
    map.insert("field_id".into(), Value::String(field_id.to_string()));
    if let Some(label) = dict_text(doc, &node.dict, b"TU") {
        let label = label.trim().to_string();
        if !label.is_empty() {
            map.insert("label".into(), Value::String(label));
        }
    }
    match node.ft.as_str() {
        "/Tx" => {
            map.insert("type".into(), Value::String("text".into()));
        }
        "/Btn" => {
            map.insert("type".into(), Value::String("checkbox".into()));
            let states = appearance_states(doc, &node.dict);
            if states.len() == 2 {
                if states.iter().any(|state| state == "/Off") {
                    let checked = states
                        .iter()
                        .find(|state| state.as_str() != "/Off")
                        .cloned();
                    if let Some(checked) = checked {
                        map.insert("checked_value".into(), Value::String(checked));
                        map.insert("unchecked_value".into(), Value::String("/Off".into()));
                    }
                } else {
                    println!(
                        "Unexpected state values for checkbox `${}`. Its checked and unchecked values may not be correct; if you're trying to check it, visually verify the results.",
                        node.qualified
                    );
                    map.insert("checked_value".into(), Value::String(states[0].clone()));
                    map.insert("unchecked_value".into(), Value::String(states[1].clone()));
                }
            }
        }
        "/Ch" => {
            map.insert("type".into(), Value::String("choice".into()));
            map.insert(
                "choice_options".into(),
                Value::Array(choice_options(doc, &node.dict)),
            );
        }
        other => {
            map.insert("type".into(), Value::String(format!("unknown ({other})")));
        }
    }
    map.insert("page".into(), Value::from(page));
    Value::Object(map)
}

fn apply_field_values(doc: &mut Document, values: &[Value]) -> PdfResult<()> {
    let nodes = collect_fields(doc)?;
    for field in values {
        let Some(value) = field.get("value").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(field_id) = field.get("field_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(node) = nodes.iter().find(|node| {
            node.qualified == field_id || strip_xfa_prefix(&node.qualified) == field_id
        }) else {
            continue;
        };
        let is_name = node.ft == "/Btn" || value.starts_with('/');
        set_value(doc, node.id, value, is_name)?;
    }
    Ok(())
}

fn set_value(doc: &mut Document, id: ObjectId, value: &str, is_name: bool) -> PdfResult<()> {
    let object = doc.get_object_mut(id).map_err(err)?;
    let dict = match object {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => &mut stream.dict,
        _ => return Err(format!("field {id:?} is not a dictionary")),
    };
    if is_name {
        let bare = value.trim_start_matches('/');
        dict.set("V", Object::Name(bare.as_bytes().to_vec()));
        dict.set("AS", Object::Name(bare.as_bytes().to_vec()));
    } else {
        dict.set("V", pdf_string(value));
    }
    Ok(())
}

fn set_need_appearances(doc: &mut Document) -> PdfResult<()> {
    let catalog = catalog_id(doc)?;
    let acro_ref = {
        let dict = owned_dict_at(doc, catalog)?;
        dict.get(b"AcroForm").ok().cloned()
    };
    let Some(acro_ref) = acro_ref else {
        return Ok(());
    };
    match acro_ref {
        Object::Reference(id) => {
            let object = doc.get_object_mut(id).map_err(err)?;
            let dict = match object {
                Object::Dictionary(dict) => dict,
                Object::Stream(stream) => &mut stream.dict,
                _ => return Err("AcroForm is not a dictionary".into()),
            };
            dict.set("NeedAppearances", true);
        }
        Object::Dictionary(mut dict) => {
            dict.set("NeedAppearances", true);
            let catalog = doc.catalog_mut().map_err(err)?;
            catalog.set("AcroForm", Object::Dictionary(dict));
        }
        _ => return Err("AcroForm is not a dictionary".into()),
    }
    Ok(())
}

fn collect_fields(doc: &Document) -> PdfResult<Vec<FieldNode>> {
    let mut out = Vec::new();
    for root in acro_field_roots(doc)? {
        walk_field(doc, root, None, &mut out)?;
    }
    Ok(out)
}

fn walk_field(
    doc: &Document,
    id: ObjectId,
    prefix: Option<&str>,
    out: &mut Vec<FieldNode>,
) -> PdfResult<()> {
    let dict = owned_dict_at(doc, id)?;
    let own = dict_text(doc, &dict, b"T");
    let qualified = join_name(prefix, own.as_deref());
    let kids = kid_ids(&dict);
    let ft = dict_pdf_name(&dict, b"FT").unwrap_or_default();
    if kids.is_empty() || ft == "/Btn" {
        out.push(FieldNode {
            id,
            qualified,
            ft,
            kids,
            dict,
        });
        return Ok(());
    }
    for kid in kids {
        walk_field(doc, kid, Some(qualified.as_str()), out)?;
    }
    Ok(())
}

fn acro_field_roots(doc: &Document) -> PdfResult<Vec<ObjectId>> {
    let catalog = catalog_id(doc)?;
    let cat = owned_dict_at(doc, catalog)?;
    let Ok(acro_obj) = cat.get(b"AcroForm") else {
        return Ok(Vec::new());
    };
    let acro = owned_dict(doc, acro_obj)?;
    let Ok(fields_obj) = acro.get(b"Fields") else {
        return Ok(Vec::new());
    };
    let array = match fields_obj {
        Object::Reference(id) => doc
            .get_object(*id)
            .map_err(err)?
            .as_array()
            .map_err(err)?
            .clone(),
        other => other.as_array().map_err(err)?.clone(),
    };
    Ok(array
        .iter()
        .filter_map(|item| item.as_reference().ok())
        .collect())
}

fn full_name_from_dict(doc: &Document, start: &Dictionary) -> String {
    let mut parts = Vec::new();
    let mut current = start.clone();
    for _ in 0..32 {
        if let Some(name) = dict_text(doc, &current, b"T") {
            if !name.is_empty() {
                parts.push(name);
            }
        }
        let Ok(parent) = current.get(b"Parent") else {
            break;
        };
        let Ok(next) = owned_dict(doc, parent) else {
            break;
        };
        current = next;
    }
    parts.reverse();
    parts.join(".")
}

fn appearance_states(doc: &Document, dict: &Dictionary) -> Vec<String> {
    let Ok(ap) = dict.get(b"AP") else {
        return Vec::new();
    };
    let Ok(ap_dict) = owned_dict(doc, ap) else {
        return Vec::new();
    };
    let Ok(normal) = ap_dict.get(b"N") else {
        return Vec::new();
    };
    if is_stream(doc, normal) {
        return Vec::new();
    }
    let Ok(states) = owned_dict(doc, normal) else {
        return Vec::new();
    };
    let mut names: Vec<String> = states
        .as_hashmap()
        .keys()
        .map(|key| format!("/{}", String::from_utf8_lossy(key)))
        .collect();
    names.sort();
    names
}

fn choice_options(doc: &Document, dict: &Dictionary) -> Vec<Value> {
    let Ok(opt) = dict.get(b"Opt") else {
        return Vec::new();
    };
    let array = match opt {
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|obj| obj.as_array().ok())
            .cloned()
            .unwrap_or_default(),
        other => other.as_array().cloned().unwrap_or_default(),
    };
    array
        .iter()
        .filter_map(|item| match item {
            Object::Array(pair) if pair.len() == 2 => {
                let value = referenced_text(doc, &pair[0])?;
                let text = referenced_text(doc, &pair[1]).unwrap_or_else(|| value.clone());
                let mut map = Map::new();
                map.insert("value".into(), Value::String(value));
                map.insert("text".into(), Value::String(text));
                Some(Value::Object(map))
            }
            other => {
                let value = referenced_text(doc, other)?;
                let mut map = Map::new();
                map.insert("value".into(), Value::String(value.clone()));
                map.insert("text".into(), Value::String(value));
                Some(Value::Object(map))
            }
        })
        .collect()
}

fn referenced_text(doc: &Document, obj: &Object) -> Option<String> {
    if let Ok(id) = obj.as_reference() {
        return doc.get_object(id).ok().and_then(obj_text);
    }
    obj_text(obj)
}

fn is_stream(doc: &Document, obj: &Object) -> bool {
    match obj {
        Object::Stream(_) => true,
        Object::Reference(id) => matches!(doc.get_object(*id), Ok(Object::Stream(_))),
        _ => false,
    }
}

fn dict_text(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<String> {
    let obj = dict.get(key).ok()?;
    referenced_text(doc, obj)
}

fn dict_pdf_name(dict: &Dictionary, key: &[u8]) -> Option<String> {
    let bytes = dict.get(key).ok()?.as_name().ok()?;
    Some(format!("/{}", String::from_utf8_lossy(bytes)))
}

fn join_name(prefix: Option<&str>, own: Option<&str>) -> String {
    match (
        prefix.filter(|text| !text.is_empty()),
        own.filter(|text| !text.is_empty()),
    ) {
        (Some(prefix), Some(own)) => format!("{prefix}.{own}"),
        (None, Some(own)) => own.to_string(),
        (Some(prefix), None) => prefix.to_string(),
        (None, None) => String::new(),
    }
}

fn strip_xfa_prefix(field_id: &str) -> String {
    match field_id.rsplit_once('.') {
        Some((_, last)) => last.to_string(),
        None => field_id.to_string(),
    }
}

pub fn page_box(doc: &Document, page_number: u32) -> PdfResult<(f64, f64)> {
    let page_id = doc
        .get_pages()
        .get(&page_number)
        .copied()
        .ok_or_else(|| format!("page {page_number} is missing"))?;
    page_size(doc, page_id)
}

/// Top-origin box to PDF rectangle (left, bottom, right, top).
pub fn rect_from_top_origin(bbox: [f64; 4], pdf_height: f64) -> [f64; 4] {
    let left = bbox[0];
    let right = bbox[2];
    let bottom = pdf_height - bbox[3];
    let top = pdf_height - bbox[1];
    [left, bottom, right, top]
}

pub fn rect_from_image_pixels(
    bbox: [f64; 4],
    image_width: f64,
    image_height: f64,
    pdf_width: f64,
    pdf_height: f64,
) -> [f64; 4] {
    let x_scale = pdf_width / image_width;
    let y_scale = pdf_height / image_height;
    let left = bbox[0] * x_scale;
    let right = bbox[2] * x_scale;
    let top = pdf_height - (bbox[1] * y_scale);
    let bottom = pdf_height - (bbox[3] * y_scale);
    [left, bottom, right, top]
}

#[cfg(test)]
mod tests {
    use super::{rect_from_image_pixels, rect_from_top_origin, strip_xfa_prefix};

    #[test]
    fn xfa_prefix_keeps_the_last_segment() {
        assert_eq!(
            strip_xfa_prefix("topmostSubform[0].Page1[0].f1_01[0]"),
            "f1_01[0]"
        );
        assert_eq!(strip_xfa_prefix("f1_01[0]"), "f1_01[0]");
    }

    #[test]
    fn top_origin_box_becomes_pdf_rect() {
        let rect = rect_from_top_origin([92.0, 63.0, 260.0, 79.0], 792.0);
        assert_eq!(rect, [92.0, 713.0, 260.0, 729.0]);
    }

    #[test]
    fn image_pixels_scale_into_pdf_rect() {
        let rect = rect_from_image_pixels([100.0, 50.0, 200.0, 80.0], 1000.0, 1000.0, 500.0, 500.0);
        assert_eq!(rect[0], 50.0);
        assert_eq!(rect[2], 100.0);
        assert!((rect[1] - 460.0).abs() < 0.01);
        assert!((rect[3] - 475.0).abs() < 0.01);
    }
}
