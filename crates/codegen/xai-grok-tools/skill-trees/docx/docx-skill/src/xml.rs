//! Small XML DOM for Office Open XML. Standard library only.

use std::path::Path;

#[derive(Clone, Debug)]
pub struct Document {
    pub decl: String,
    pub root: Element,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Element(Element),
    Text(String),
    Comment(String),
}

pub fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub fn parse(input: &str) -> Result<Document, String> {
    let mut p = Parser { s: input, i: 0 };
    if p.s.starts_with('\u{feff}') {
        p.i = '\u{feff}'.len_utf8();
    }
    let mut decl = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    p.skip_ws();
    if p.starts_with("<?") {
        let start = p.i;
        p.i += 2;
        if let Some(end) = p.s[p.i..].find("?>") {
            decl = p.s[start..p.i + end + 2].to_string();
            p.i += end + 2;
        } else {
            return Err("unterminated xml declaration".into());
        }
    }
    let root = p.parse_element()?;
    Ok(Document { decl, root })
}

pub fn parse_file(path: &Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse(&text)
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.s[self.i..].chars().next() {
            if c.is_whitespace() {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn parse_element(&mut self) -> Result<Element, String> {
        self.skip_misc();
        if !self.starts_with("<") {
            return Err("expected element".into());
        }
        self.i += 1;
        let name = self.read_name()?;
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            if self.starts_with("/>") {
                self.i += 2;
                return Ok(Element {
                    name,
                    attrs,
                    children: Vec::new(),
                });
            }
            if self.starts_with(">") {
                self.i += 1;
                break;
            }
            let an = self.read_name()?;
            self.skip_ws();
            if !self.starts_with("=") {
                return Err(format!("attribute {an} missing value"));
            }
            self.i += 1;
            self.skip_ws();
            let av = self.read_attr_value()?;
            attrs.push((an, decode_entities(&av)));
        }
        let mut children = Vec::new();
        loop {
            if self.i >= self.s.len() {
                return Err(format!("unclosed <{name}>"));
            }
            if self.starts_with("</") {
                self.i += 2;
                let end = self.read_name()?;
                self.skip_ws();
                if !self.starts_with(">") {
                    return Err("bad end tag".into());
                }
                self.i += 1;
                if end != name {
                    return Err(format!("end tag {end} does not close {name}"));
                }
                break;
            }
            if self.starts_with("<!--") {
                self.i += 4;
                if let Some(end) = self.s[self.i..].find("-->") {
                    children.push(Node::Comment(self.s[self.i..self.i + end].to_string()));
                    self.i += end + 3;
                } else {
                    return Err("unterminated comment".into());
                }
                continue;
            }
            if self.starts_with("<![CDATA[") {
                self.i += 9;
                if let Some(end) = self.s[self.i..].find("]]>") {
                    children.push(Node::Text(self.s[self.i..self.i + end].to_string()));
                    self.i += end + 3;
                } else {
                    return Err("unterminated cdata".into());
                }
                continue;
            }
            if self.starts_with("<?") {
                self.i += 2;
                if let Some(end) = self.s[self.i..].find("?>") {
                    self.i += end + 2;
                } else {
                    return Err("unterminated processing instruction".into());
                }
                continue;
            }
            if self.starts_with("<") {
                children.push(Node::Element(self.parse_element()?));
                continue;
            }
            let start = self.i;
            if let Some(rel) = self.s[self.i..].find('<') {
                let text = &self.s[start..self.i + rel];
                self.i += rel;
                children.push(Node::Text(decode_entities(text)));
            } else {
                return Err("text ran off the end".into());
            }
        }
        Ok(Element {
            name,
            attrs,
            children,
        })
    }

    fn skip_misc(&mut self) {
        loop {
            self.skip_ws();
            if self.starts_with("<!--") {
                self.i += 4;
                if let Some(end) = self.s[self.i..].find("-->") {
                    self.i += end + 3;
                } else {
                    return;
                }
            } else if self.starts_with("<?") {
                self.i += 2;
                if let Some(end) = self.s[self.i..].find("?>") {
                    self.i += end + 2;
                } else {
                    return;
                }
            } else {
                return;
            }
        }
    }

    fn read_name(&mut self) -> Result<String, String> {
        let start = self.i;
        for (k, c) in self.s[self.i..].char_indices() {
            if c.is_whitespace() || c == '/' || c == '>' || c == '=' {
                if k == 0 {
                    return Err("empty name".into());
                }
                self.i += k;
                return Ok(self.s[start..self.i].to_string());
            }
        }
        Err("name ran off the end".into())
    }

    fn read_attr_value(&mut self) -> Result<String, String> {
        let quote = self.s[self.i..].chars().next().ok_or("attr value")?;
        if quote != '"' && quote != '\'' {
            return Err("attr value not quoted".into());
        }
        self.i += quote.len_utf8();
        let start = self.i;
        if let Some(rel) = self.s[self.i..].find(quote) {
            let v = self.s[start..self.i + rel].to_string();
            self.i += rel + quote.len_utf8();
            Ok(v)
        } else {
            Err("unterminated attr".into())
        }
    }
}

pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' {
            if let Some(rel) = s[i..].find(';') {
                let ent = &s[i + 1..i + rel];
                let ch = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                    }
                    _ if ent.starts_with('#') => {
                        ent[1..].parse::<u32>().ok().and_then(char::from_u32)
                    }
                    _ => None,
                };
                if let Some(ch) = ch {
                    out.push(ch);
                    i += rel + 1;
                    continue;
                }
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name || local_name(n) == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        if let Some(slot) = self
            .attrs
            .iter_mut()
            .find(|(n, _)| n == name || local_name(n) == name)
        {
            slot.1 = value.to_string();
        } else {
            self.attrs.push((name.to_string(), value.to_string()));
        }
    }

    pub fn remove_attr_matching<F: Fn(&str) -> bool>(&mut self, pred: F) {
        self.attrs.retain(|(n, _)| !pred(n));
    }

    pub fn local(&self) -> &str {
        local_name(&self.name)
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn child(&self, tag: &str) -> Option<&Element> {
        self.elements().find(|e| e.local() == tag || e.name == tag)
    }

    pub fn text_content(&self) -> String {
        let mut s = String::new();
        fn walk(n: &Node, s: &mut String) {
            match n {
                Node::Text(t) => s.push_str(t),
                Node::Element(e) => {
                    for c in &e.children {
                        walk(c, s);
                    }
                }
                Node::Comment(_) => {}
            }
        }
        for c in &self.children {
            walk(c, &mut s);
        }
        s
    }
}

pub fn write_pretty(doc: &Document) -> String {
    let mut out = String::new();
    out.push_str(&doc.decl);
    out.push('\n');
    write_el(&doc.root, 0, &mut out, true, false);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn write_condensed(doc: &Document) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    write_el(&doc.root, 0, &mut out, false, true);
    out
}

pub fn serialize_el(el: &Element) -> String {
    let mut out = String::new();
    write_el(el, 0, &mut out, false, true);
    out
}

pub fn write_el(el: &Element, indent: usize, out: &mut String, pretty: bool, condense: bool) {
    if pretty {
        for _ in 0..indent {
            out.push_str("  ");
        }
    }
    out.push('<');
    out.push_str(&el.name);
    for (n, v) in &el.attrs {
        out.push(' ');
        out.push_str(n);
        out.push_str("=\"");
        out.push_str(&escape_attr(v));
        out.push('"');
    }
    let keep_text = local_name(&el.name) == "t";
    let drop_ws = (pretty || condense) && !keep_text;
    let meaningful: Vec<&Node> = el
        .children
        .iter()
        .filter(|n| match n {
            Node::Text(t) if drop_ws && t.trim().is_empty() => false,
            Node::Comment(_) if drop_ws => false,
            _ => true,
        })
        .collect();
    if meaningful.is_empty() {
        out.push_str("/>");
        if pretty {
            out.push('\n');
        }
        return;
    }
    out.push('>');
    let only_text = meaningful.iter().all(|n| matches!(n, Node::Text(_)));
    if pretty && !only_text {
        out.push('\n');
    }
    for n in meaningful {
        match n {
            Node::Text(t) => {
                if pretty && !only_text && !keep_text {
                    for _ in 0..indent + 1 {
                        out.push_str("  ");
                    }
                }
                out.push_str(&escape_text(t));
                if pretty && !only_text && !keep_text {
                    out.push('\n');
                }
            }
            Node::Comment(c) => {
                if pretty {
                    for _ in 0..indent + 1 {
                        out.push_str("  ");
                    }
                }
                out.push_str("<!--");
                out.push_str(c);
                out.push_str("-->");
                if pretty {
                    out.push('\n');
                }
            }
            Node::Element(child) => write_el(child, indent + 1, out, pretty, condense),
        }
    }
    if pretty && !only_text {
        for _ in 0..indent {
            out.push_str("  ");
        }
    }
    out.push_str("</");
    out.push_str(&el.name);
    out.push('>');
    if pretty {
        out.push('\n');
    }
}

pub fn find_all<'a>(el: &'a Element, tag: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    fn walk<'a>(e: &'a Element, tag: &str, out: &mut Vec<&'a Element>) {
        for c in e.elements() {
            if c.local() == tag {
                out.push(c);
            }
            walk(c, tag, out);
        }
    }
    walk(el, tag, &mut out);
    out
}

pub fn remove_named(el: &mut Element, tag: &str) {
    el.children.retain(|n| match n {
        Node::Element(e) => e.local() != tag,
        _ => true,
    });
    for i in 0..el.children.len() {
        if let Node::Element(child) = &mut el.children[i] {
            remove_named(child, tag);
        }
    }
}

pub fn for_each_mut(el: &mut Element, f: &mut dyn FnMut(&mut Element)) {
    f(el);
    for i in 0..el.children.len() {
        if let Node::Element(child) = &mut el.children[i] {
            for_each_mut(child, f);
        }
    }
}

pub fn smart_quote_entities(s: &str) -> String {
    s.replace('\u{201c}', "&#x201C;")
        .replace('\u{201d}', "&#x201D;")
        .replace('\u{2018}', "&#x2018;")
        .replace('\u{2019}', "&#x2019;")
}
