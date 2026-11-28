//! Bounding-box checks for flat-form fields.json. Same messages as the old script.

use serde_json::Value;

use crate::pdf_obj::{usage, PdfResult};

struct RectField {
    index: usize,
    rect: [JsonNum; 4],
    rect_type: &'static str,
}

#[derive(Clone, Copy)]
enum JsonNum {
    Int(i64),
    Uint(u64),
    Float(f64),
}

impl JsonNum {
    fn from_value(value: &Value) -> PdfResult<Self> {
        let num = value
            .as_number()
            .ok_or_else(|| "bounding box entry is not a number".to_string())?;
        if num.is_i64() {
            let i = num
                .as_i64()
                .ok_or_else(|| "bounding box entry is not a number".to_string())?;
            return Ok(JsonNum::Int(i));
        }
        if num.is_u64() {
            let u = num
                .as_u64()
                .ok_or_else(|| "bounding box entry is not a number".to_string())?;
            return Ok(JsonNum::Uint(u));
        }
        let f = num
            .as_f64()
            .ok_or_else(|| "bounding box entry is not a number".to_string())?;
        Ok(JsonNum::Float(f))
    }

    fn as_f64(self) -> f64 {
        match self {
            JsonNum::Int(i) => i as f64,
            JsonNum::Uint(u) => u as f64,
            JsonNum::Float(f) => f,
        }
    }

    /// Text form of a JSON number (`14` versus `14.0`).
    fn py_str(self) -> String {
        match self {
            JsonNum::Int(i) => i.to_string(),
            JsonNum::Uint(u) => u.to_string(),
            JsonNum::Float(f) => py_float(f),
        }
    }
}

fn py_float(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    let debug = format!("{value:?}");
    if debug.contains('.') || debug.contains('e') || debug.contains('E') {
        debug
    } else {
        format!("{debug}.0")
    }
}

fn py_sub(left: JsonNum, right: JsonNum) -> JsonNum {
    match (left, right) {
        (JsonNum::Int(a), JsonNum::Int(b)) => match a.checked_sub(b) {
            Some(value) => JsonNum::Int(value),
            None => JsonNum::Float(left.as_f64() - right.as_f64()),
        },
        (JsonNum::Uint(a), JsonNum::Uint(b)) if a >= b => JsonNum::Uint(a - b),
        _ => JsonNum::Float(left.as_f64() - right.as_f64()),
    }
}

fn rect_from(value: &Value) -> PdfResult<[JsonNum; 4]> {
    let arr = value
        .as_array()
        .ok_or_else(|| "bounding box is not an array".to_string())?;
    if arr.len() != 4 {
        return Err("bounding box must have 4 numbers".into());
    }
    let mut out = [JsonNum::Int(0); 4];
    for (i, item) in arr.iter().enumerate() {
        out[i] = JsonNum::from_value(item)?;
    }
    Ok(out)
}

fn fmt_rect(rect: &[JsonNum; 4]) -> String {
    format!(
        "[{}, {}, {}, {}]",
        rect[0].py_str(),
        rect[1].py_str(),
        rect[2].py_str(),
        rect[3].py_str()
    )
}

fn intersects(a: [JsonNum; 4], b: [JsonNum; 4]) -> bool {
    let a = [a[0].as_f64(), a[1].as_f64(), a[2].as_f64(), a[3].as_f64()];
    let b = [b[0].as_f64(), b[1].as_f64(), b[2].as_f64(), b[3].as_f64()];
    let disjoint_h = a[0] >= b[2] || a[2] <= b[0];
    let disjoint_v = a[1] >= b[3] || a[3] <= b[1];
    !(disjoint_h || disjoint_v)
}

fn page_of(field: &Value) -> PdfResult<f64> {
    let value = field
        .get("page_number")
        .ok_or_else(|| "field is missing page_number".to_string())?;
    value
        .as_f64()
        .ok_or_else(|| "page_number is not a number".to_string())
}

fn description(field: &Value) -> PdfResult<String> {
    field
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "field is missing description".to_string())
}

fn push_abort(messages: &mut Vec<String>) -> bool {
    if messages.len() >= 20 {
        messages.push("Aborting further checks; fix bounding boxes and try again".into());
        true
    } else {
        false
    }
}

pub fn bounding_box_messages(fields: &Value) -> PdfResult<Vec<String>> {
    let form_fields = fields
        .get("form_fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "fields json has no form_fields array".to_string())?;
    let mut messages = vec![format!("Read {} fields", form_fields.len())];
    let mut rects = Vec::new();
    for (index, field) in form_fields.iter().enumerate() {
        let label = rect_from(
            field
                .get("label_bounding_box")
                .ok_or_else(|| format!("field {index} has no label_bounding_box"))?,
        )?;
        let entry = rect_from(
            field
                .get("entry_bounding_box")
                .ok_or_else(|| format!("field {index} has no entry_bounding_box"))?,
        )?;
        rects.push(RectField {
            index,
            rect: label,
            rect_type: "label",
        });
        rects.push(RectField {
            index,
            rect: entry,
            rect_type: "entry",
        });
    }

    let mut has_error = false;
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let left = &rects[i];
            let right = &rects[j];
            let left_field = &form_fields[left.index];
            let right_field = &form_fields[right.index];
            if page_of(left_field)? != page_of(right_field)? {
                continue;
            }
            if !intersects(left.rect, right.rect) {
                continue;
            }
            has_error = true;
            if left.index == right.index {
                messages.push(format!(
                    "FAILURE: intersection between label and entry bounding boxes for `{}` ({}, {})",
                    description(left_field)?,
                    fmt_rect(&left.rect),
                    fmt_rect(&right.rect)
                ));
            } else {
                messages.push(format!(
                    "FAILURE: intersection between {} bounding box for `{}` ({}) and {} bounding box for `{}` ({})",
                    left.rect_type,
                    description(left_field)?,
                    fmt_rect(&left.rect),
                    right.rect_type,
                    description(right_field)?,
                    fmt_rect(&right.rect)
                ));
            }
            if push_abort(&mut messages) {
                return Ok(messages);
            }
        }
        let ri = &rects[i];
        if ri.rect_type != "entry" {
            continue;
        }
        let field = &form_fields[ri.index];
        let Some(entry_text) = field.get("entry_text") else {
            continue;
        };
        if !entry_text.is_object() {
            return Err("entry_text is not an object".into());
        }
        let font_size = match entry_text.get("font_size") {
            None => JsonNum::Int(14),
            Some(value) => JsonNum::from_value(value)?,
        };
        let entry_height = py_sub(ri.rect[3], ri.rect[1]);
        if entry_height.as_f64() < font_size.as_f64() {
            has_error = true;
            messages.push(format!(
                "FAILURE: entry bounding box height ({}) for `{}` is too short for the text content (font size: {}). Increase the box height or decrease the font size.",
                entry_height.py_str(),
                description(field)?,
                font_size.py_str()
            ));
            if push_abort(&mut messages) {
                return Ok(messages);
            }
        }
    }
    if !has_error {
        messages.push("SUCCESS: All bounding boxes are valid".into());
    }
    Ok(messages)
}

pub fn check_bounding_boxes_main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        usage("Usage: check_bounding_boxes [fields.json]");
    }
    let file = std::fs::File::open(&args[0]).map_err(|err| err.to_string())?;
    let fields: Value = serde_json::from_reader(file).map_err(|err| err.to_string())?;
    for msg in bounding_box_messages(&fields)? {
        println!("{msg}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::bounding_box_messages;
    use serde_json::json;

    #[test]
    fn disjoint_boxes_succeed() {
        let fields = json!({
            "form_fields": [{
                "page_number": 1,
                "description": "Last name",
                "label_bounding_box": [10, 10, 40, 20],
                "entry_bounding_box": [50, 10, 120, 30],
                "entry_text": {"text": "Smith", "font_size": 10}
            }]
        });
        let messages = bounding_box_messages(&fields).expect("parse");
        assert_eq!(messages[0], "Read 1 fields");
        assert_eq!(messages[1], "SUCCESS: All bounding boxes are valid");
    }

    #[test]
    fn short_entry_fails() {
        let fields = json!({
            "form_fields": [{
                "page_number": 1,
                "description": "Name",
                "label_bounding_box": [10, 10, 30, 20],
                "entry_bounding_box": [40, 10, 80, 16],
                "entry_text": {"font_size": 14}
            }]
        });
        let messages = bounding_box_messages(&fields).expect("parse");
        assert_eq!(
            messages[1],
            "FAILURE: entry bounding box height (6) for `Name` is too short for the text content (font size: 14). Increase the box height or decrease the font size."
        );
    }

    #[test]
    fn same_field_overlap_prints_integer_rects() {
        let fields = json!({
            "form_fields": [{
                "page_number": 1,
                "description": "Name",
                "label_bounding_box": [0, 0, 10, 10],
                "entry_bounding_box": [5, 5, 15, 20],
                "entry_text": {"font_size": 14}
            }]
        });
        let messages = bounding_box_messages(&fields).expect("parse");
        assert_eq!(
            messages[1],
            "FAILURE: intersection between label and entry bounding boxes for `Name` ([0, 0, 10, 10], [5, 5, 15, 20])"
        );
    }

    #[test]
    fn cross_field_overlap_names_each_box() {
        let fields = json!({
            "form_fields": [
                {
                    "page_number": 1,
                    "description": "A",
                    "label_bounding_box": [0, 0, 1, 1],
                    "entry_bounding_box": [10, 10, 20, 20]
                },
                {
                    "page_number": 1,
                    "description": "B",
                    "label_bounding_box": [2, 2, 3, 3],
                    "entry_bounding_box": [15, 15, 25, 25]
                }
            ]
        });
        let messages = bounding_box_messages(&fields).expect("parse");
        assert_eq!(
            messages[1],
            "FAILURE: intersection between entry bounding box for `A` ([10, 10, 20, 20]) and entry bounding box for `B` ([15, 15, 25, 25])"
        );
    }

    #[test]
    fn float_height_keeps_a_decimal() {
        let fields = json!({
            "form_fields": [{
                "page_number": 1,
                "description": "Name",
                "label_bounding_box": [0, 0, 1, 1],
                "entry_bounding_box": [40, 10, 80, 16.0],
                "entry_text": {"font_size": 14}
            }]
        });
        let messages = bounding_box_messages(&fields).expect("parse");
        assert!(messages
            .iter()
            .any(|m| m.contains("height (6.0)") && m.contains("font size: 14")));
    }
}
