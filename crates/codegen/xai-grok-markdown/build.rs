use std::env;
use std::path::Path;

use serde_json::{Map, Value, json};
use syntect::dumps::dump_to_uncompressed_file;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlLoader};

// `__start` must not include prototypes. Syntect injects these two contexts
// so popping `main` stays inside the file scope.
const START_CONTEXT: &str = "
__start:
    - meta_include_prototype: false
    - match: ''
      push: __main
__main:
    - include: main
";

fn main() {
    println!("cargo:rerun-if-changed=assets/Swift.sublime-syntax");
    let swift = load_swift_syntax(include_str!("assets/Swift.sublime-syntax"));
    let mut builder = two_face::syntax::extra_newlines().into_builder();
    builder.add(swift);
    let set = builder.build();
    dump_to_uncompressed_file(
        &set,
        Path::new(&env::var("OUT_DIR").unwrap()).join("syntaxes.bin"),
    )
    .expect("dump syntaxes");
}

fn load_swift_syntax(source: &str) -> syntect::parsing::SyntaxDefinition {
    let doc = first_document(source);
    let root = doc.as_hash().expect("Swift syntax root is a mapping");
    let name = yaml_str(root, "name").unwrap_or("Unnamed");
    let scope = yaml_str(root, "scope").expect("Swift syntax scope");
    let contexts_hash = yaml_key(root, "contexts")
        .and_then(Yaml::as_hash)
        .expect("Swift syntax contexts");

    let mut contexts = Map::new();
    for (key, value) in contexts_hash {
        let (Some(context_name), Some(rules)) = (key.as_str(), value.as_vec()) else {
            continue;
        };
        let is_prototype = context_name == "prototype";
        let mut namer = ContextNamer::new(context_name);
        parse_context(rules, &mut contexts, &mut namer, is_prototype);
    }
    if !contexts.contains_key("main") {
        panic!("Swift syntax is missing the main context");
    }
    add_initial_contexts(&mut contexts, scope);

    let mut file_extensions = Vec::new();
    for key in ["file_extensions", "hidden_file_extensions"] {
        if let Some(values) = yaml_key(root, key).and_then(Yaml::as_vec) {
            file_extensions.extend(
                values
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned)),
            );
        }
    }
    let first_line_match = yaml_str(root, "first_line_match");
    let hidden = yaml_key(root, "hidden")
        .and_then(Yaml::as_bool)
        .unwrap_or(false);

    let value = json!({
        "name": name,
        "file_extensions": file_extensions,
        "scope": scope,
        "first_line_match": first_line_match,
        "hidden": hidden,
        "variables": {},
        "contexts": contexts,
    });
    serde_json::from_value(value).unwrap_or_else(|err| panic!("Swift syntax definition: {err}"))
}

fn first_document(source: &str) -> Yaml {
    let docs = YamlLoader::load_from_str(source).unwrap_or_else(|err| {
        let stripped: String = source
            .lines()
            .filter(|line| !line.starts_with('%'))
            .collect::<Vec<_>>()
            .join("\n");
        YamlLoader::load_from_str(&stripped)
            .unwrap_or_else(|_| panic!("parse Swift.sublime-syntax: {err}"))
    });
    docs.into_iter().next().expect("Swift syntax document")
}

fn add_initial_contexts(contexts: &mut Map<String, Value>, top_level_scope: &str) {
    let injected = first_document(START_CONTEXT);
    let injected = injected.as_hash().expect("injected syntax contexts");
    let start_rules = yaml_key(injected, "__start")
        .and_then(Yaml::as_vec)
        .expect("__start");
    let mut start_namer = ContextNamer::new("__start");
    parse_context(start_rules, contexts, &mut start_namer, false);
    if let Some(start) = contexts.get_mut("__start") {
        start["meta_content_scope"] = json!([top_level_scope]);
    }

    let main_rules = yaml_key(injected, "__main")
        .and_then(Yaml::as_vec)
        .expect("__main");
    let mut main_namer = ContextNamer::new("__main");
    parse_context(main_rules, contexts, &mut main_namer, false);

    let main = contexts.get("main").expect("main context").clone();
    if let Some(outer_main) = contexts.get_mut("__main") {
        outer_main["meta_include_prototype"] = main["meta_include_prototype"].clone();
        outer_main["meta_scope"] = main["meta_scope"].clone();
        outer_main["meta_content_scope"] = main["meta_content_scope"].clone();
    }
    if let Some(main) = contexts.get_mut("main")
        && let Some(scopes) = main["meta_content_scope"].as_array_mut()
    {
        scopes.insert(0, json!(top_level_scope));
    }
}

struct ContextNamer {
    name: String,
    anonymous_index: Option<usize>,
}

impl ContextNamer {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            anonymous_index: None,
        }
    }

    fn next(&mut self) -> String {
        let name = if let Some(index) = self.anonymous_index {
            format!("#anon_{}_{index}", self.name)
        } else {
            self.name.clone()
        };
        self.anonymous_index = Some(self.anonymous_index.map(|index| index + 1).unwrap_or(0));
        name
    }
}

struct ContextBuild {
    meta_scope: Vec<String>,
    meta_content_scope: Vec<String>,
    meta_include_prototype: bool,
    uses_backrefs: bool,
    patterns: Vec<Value>,
}

fn parse_context(
    rules: &[Yaml],
    contexts: &mut Map<String, Value>,
    namer: &mut ContextNamer,
    is_prototype: bool,
) -> String {
    let mut context = ContextBuild {
        meta_scope: Vec::new(),
        meta_content_scope: Vec::new(),
        meta_include_prototype: !is_prototype,
        uses_backrefs: false,
        patterns: Vec::new(),
    };
    let name = namer.next();
    for rule in rules {
        let map = rule.as_hash().expect("syntax rule is a mapping");
        let mut is_special = false;
        if let Some(scope) = yaml_str(map, "meta_scope") {
            context.meta_scope = split_scopes(scope);
            is_special = true;
        }
        if let Some(scope) = yaml_str(map, "meta_content_scope") {
            context.meta_content_scope = split_scopes(scope);
            is_special = true;
        }
        if let Some(include_prototype) =
            yaml_key(map, "meta_include_prototype").and_then(Yaml::as_bool)
        {
            context.meta_include_prototype = include_prototype;
            is_special = true;
        }
        if is_special {
            continue;
        }
        if let Some(include) = yaml_key(map, "include") {
            context.patterns.push(json!({
                "Include": parse_reference(include, contexts, namer),
            }));
            continue;
        }
        let (pattern, has_captures) = parse_match(map, contexts, namer);
        if has_captures {
            context.uses_backrefs = true;
        }
        context.patterns.push(pattern);
    }
    contexts.insert(name.clone(), context.into_value());
    name
}

impl ContextBuild {
    fn into_value(self) -> Value {
        json!({
            "meta_scope": self.meta_scope,
            "meta_content_scope": self.meta_content_scope,
            "meta_include_prototype": self.meta_include_prototype,
            "clear_scopes": null,
            "prototype": null,
            "uses_backrefs": self.uses_backrefs,
            "patterns": self.patterns,
        })
    }
}

fn parse_reference(
    value: &Yaml,
    contexts: &mut Map<String, Value>,
    namer: &mut ContextNamer,
) -> Value {
    if let Some(reference) = value.as_str() {
        let mut parts = reference.splitn(2, '#');
        let head = parts.next().unwrap_or("");
        let sub_context = parts.next();
        if let Some(scope) = head.strip_prefix("scope:") {
            json!({
                "ByScope": {
                    "scope": scope,
                    "sub_context": sub_context,
                    "with_escape": false,
                }
            })
        } else if head.ends_with(".sublime-syntax") {
            let stem = Path::new(head)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("syntax file reference");
            json!({
                "File": {
                    "name": stem,
                    "sub_context": sub_context,
                    "with_escape": false,
                }
            })
        } else {
            json!({ "Named": head })
        }
    } else if let Some(rules) = value.as_vec() {
        let subname = parse_context(rules, contexts, namer, false);
        json!({ "Inline": subname })
    } else {
        panic!("syntax reference is not a name or an inline context");
    }
}

fn parse_match(
    map: &Hash,
    contexts: &mut Map<String, Value>,
    namer: &mut ContextNamer,
) -> (Value, bool) {
    let raw_regex = yaml_str(map, "match").expect("syntax match");
    let regex = parse_regex(raw_regex);
    let scope = yaml_str(map, "scope").map(split_scopes).unwrap_or_default();
    let captures = if let Some(captures) = yaml_key(map, "captures").and_then(Yaml::as_hash) {
        Value::Array(parse_captures(captures, &regex))
    } else {
        Value::Null
    };
    let has_captures = regex_has_backref(&regex);
    let operation = if yaml_key(map, "pop").is_some() {
        json!("Pop")
    } else if let Some(push) = yaml_key(map, "push") {
        json!({ "Push": parse_push(push, contexts, namer) })
    } else {
        json!("None")
    };
    let pattern = json!({
        "Match": {
            "has_captures": has_captures,
            "regex": regex,
            "scope": scope,
            "captures": captures,
            "operation": operation,
            "with_prototype": null,
        }
    });
    (pattern, has_captures)
}

fn parse_push(
    value: &Yaml,
    contexts: &mut Map<String, Value>,
    namer: &mut ContextNamer,
) -> Vec<Value> {
    if is_push_list(value) {
        value
            .as_vec()
            .expect("push list")
            .iter()
            .map(|item| parse_reference(item, contexts, namer))
            .collect()
    } else {
        vec![parse_reference(value, contexts, namer)]
    }
}

fn is_push_list(value: &Yaml) -> bool {
    let Some(items) = value.as_vec() else {
        return false;
    };
    let Some(first) = items.first() else {
        return false;
    };
    if first.as_str().is_some() {
        return true;
    }
    first
        .as_vec()
        .and_then(|inner| inner.first())
        .is_some_and(Yaml::is_hash)
}

fn parse_captures(map: &Hash, regex: &str) -> Vec<Value> {
    let valid = consuming_capture_indexes(regex);
    let mut captures = Vec::new();
    for (key, value) in map {
        let Some(index) = capture_index(key) else {
            continue;
        };
        if !valid.contains(&index) {
            continue;
        }
        captures.push((index, capture_scopes(value)));
    }
    captures.sort_by_key(|(index, _)| *index);
    captures
        .into_iter()
        .map(|(index, scopes)| json!([index, scopes]))
        .collect()
}

fn capture_index(key: &Yaml) -> Option<usize> {
    if let Some(index) = key.as_i64()
        && index >= 0
    {
        return Some(index as usize);
    }
    key.as_str()?.parse().ok()
}

fn capture_scopes(value: &Yaml) -> Vec<String> {
    if let Some(scope) = value.as_str() {
        return split_scopes(scope);
    }
    let Some(map) = value.as_hash() else {
        return Vec::new();
    };
    yaml_str(map, "name")
        .or_else(|| yaml_str(map, "scope"))
        .map(split_scopes)
        .unwrap_or_default()
}

fn split_scopes(scopes: &str) -> Vec<String> {
    scopes.split_whitespace().map(str::to_owned).collect()
}

fn parse_regex(raw: &str) -> String {
    let regex = replace_posix_char_classes(raw);
    let regex = regex_for_newlines(regex);
    let check = substitute_backrefs(&regex);
    if let Some(err) = syntect::parsing::Regex::try_compile(&check) {
        panic!("Swift syntax regex '{regex}': {err}");
    }
    regex
}

fn regex_has_backref(regex: &str) -> bool {
    let bytes = regex.as_bytes();
    bytes
        .windows(2)
        .any(|pair| pair[0] == b'\\' && pair[1].is_ascii_digit())
}

fn replace_posix_char_classes(regex: &str) -> String {
    regex
        .replace("[:alpha:]", r"\p{L}")
        .replace("[:alnum:]", r"\p{L}\p{N}")
        .replace("[:lower:]", r"\p{Ll}")
        .replace("[:upper:]", r"\p{Lu}")
        .replace("[:digit:]", r"\p{Nd}")
}

fn substitute_backrefs(regex: &str) -> String {
    let mut rewritten = String::with_capacity(regex.len());
    let mut last_was_escape = false;
    for ch in regex.chars() {
        if last_was_escape && ch.is_ascii_digit() {
            let index = ch.to_digit(10).expect("digit");
            rewritten.push_str(&format!("<placeholder_{index}>"));
        } else if last_was_escape {
            rewritten.push('\\');
            rewritten.push(ch);
        } else if ch != '\\' {
            rewritten.push(ch);
        }
        last_was_escape = ch == '\\' && !last_was_escape;
    }
    if last_was_escape {
        rewritten.push('\\');
    }
    rewritten
}

fn regex_for_newlines(regex: String) -> String {
    if !regex.contains('$') {
        return regex;
    }
    let mut parser = ByteParser::new(regex.as_bytes());
    let mut result = Vec::new();
    while let Some(byte) = parser.peek() {
        match byte {
            b'$' => {
                parser.next();
                result.extend_from_slice(br"(?m:$)");
            }
            b'\\' => {
                parser.next();
                result.push(byte);
                if let Some(escaped) = parser.peek() {
                    parser.next();
                    result.push(escaped);
                }
            }
            b'[' => {
                let (mut content, _) = parser.parse_character_class();
                result.append(&mut content);
            }
            _ => {
                parser.next();
                result.push(byte);
            }
        }
    }
    String::from_utf8(result).expect("rewritten regex")
}

fn consuming_capture_indexes(regex: &str) -> Vec<usize> {
    let mut parser = ByteParser::new(regex.as_bytes());
    let mut indexes = Vec::new();
    let mut stack = Vec::new();
    let mut capture_number = 0usize;
    let mut in_lookaround = false;
    stack.push(in_lookaround);
    indexes.push(capture_number);
    while let Some(byte) = parser.peek() {
        match byte {
            b'\\' => {
                parser.next();
                parser.next();
            }
            b'[' => {
                parser.parse_character_class();
            }
            b'(' => {
                parser.next();
                stack.push(in_lookaround);
                if let Some(next) = parser.peek() {
                    if next != b'?' {
                        capture_number += 1;
                        if !in_lookaround {
                            indexes.push(capture_number);
                        }
                    } else {
                        parser.next();
                        if let Some(kind) = parser.peek() {
                            parser.next();
                            if kind == b'=' || kind == b'!' {
                                in_lookaround = true;
                            } else if kind == b'<' {
                                if let Some(look) = parser.peek()
                                    && (look == b'=' || look == b'!')
                                {
                                    parser.next();
                                    in_lookaround = true;
                                }
                            } else if kind == b'P' && parser.peek() == Some(b'<') {
                                capture_number += 1;
                                if !in_lookaround {
                                    indexes.push(capture_number);
                                }
                            }
                        }
                    }
                }
            }
            b')' => {
                if let Some(previous) = stack.pop() {
                    in_lookaround = previous;
                }
                parser.next();
            }
            _ => parser.next(),
        }
    }
    indexes
}

struct ByteParser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> ByteParser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, index: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn next(&mut self) {
        self.index += 1;
    }

    fn parse_character_class(&mut self) -> (Vec<u8>, bool) {
        let mut content = Vec::new();
        let mut negated = false;
        let mut nesting = 0usize;
        let mut matches_newline = false;
        self.next();
        content.push(b'[');
        if self.peek() == Some(b'^') {
            self.next();
            content.push(b'^');
            negated = true;
        }
        if self.peek() == Some(b']') {
            self.next();
            content.push(b']');
        }
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => {
                    self.next();
                    content.push(byte);
                    if let Some(escaped) = self.peek() {
                        self.next();
                        if escaped == b'n' && !negated && nesting == 0 {
                            matches_newline = true;
                        }
                        content.push(escaped);
                    }
                }
                b'[' => {
                    self.next();
                    content.push(b'[');
                    nesting += 1;
                }
                b']' => {
                    self.next();
                    content.push(b']');
                    if nesting == 0 {
                        break;
                    }
                    nesting -= 1;
                }
                _ => {
                    self.next();
                    content.push(byte);
                }
            }
        }
        (content, matches_newline)
    }
}

fn yaml_key<'a>(map: &'a Hash, key: &str) -> Option<&'a Yaml> {
    map.get(&Yaml::String(key.to_owned()))
}

fn yaml_str<'a>(map: &'a Hash, key: &str) -> Option<&'a str> {
    yaml_key(map, key).and_then(Yaml::as_str)
}
