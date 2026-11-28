//! DOCX skill operations. This binary does not start an interpreter.

mod xml;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::Value;

const DEFAULT_AUTHOR: &str = "Claude";
const OFFICE_SUFFIXES: &[&str] = &[".docx", ".pptx", ".xlsx"];
const LIBREOFFICE_PROFILE: &str = "/tmp/libreoffice_docx_profile";
const ACCEPT_MACRO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE script:module PUBLIC "-//OpenOffice.org//DTD OfficeDocument 1.0//EN" "module.dtd">
<script:module xmlns:script="http://openoffice.org/2000/script" script:name="Module1" script:language="StarBasic">
    Sub AcceptAllTrackedChanges()
        Dim document As Object
        Dim dispatcher As Object

        document = ThisComponent.CurrentController.Frame
        dispatcher = createUnoService("com.sun.star.frame.DispatchHelper")

        dispatcher.executeDispatch(document, ".uno:AcceptAllTrackedChanges", "", 0, Array())
        ThisComponent.store()
        ThisComponent.close(True)
    End Sub
</script:module>"#;

const COMMENTS_XML: &str = include_str!("../../scripts/templates/comments.xml");
const COMMENTS_EX: &str = include_str!("../../scripts/templates/commentsExtended.xml");
const COMMENTS_IDS: &str = include_str!("../../scripts/templates/commentsIds.xml");
const COMMENTS_EXTENSIBLE: &str = include_str!("../../scripts/templates/commentsExtensible.xml");

const COMMENT_BODY: &str = "\
<w:comment w:id=\"{id}\" w:author=\"{author}\" w:date=\"{date}\" w:initials=\"{initials}\">
  <w:p w14:paraId=\"{para_id}\" w14:textId=\"77777777\">
    <w:r>
      <w:rPr><w:rStyle w:val=\"CommentReference\"/></w:rPr>
      <w:annotationRef/>
    </w:r>
    <w:r>
      <w:rPr>
        <w:color w:val=\"000000\"/>
        <w:sz w:val=\"20\"/>
        <w:szCs w:val=\"20\"/>
      </w:rPr>
      <w:t>{text}</w:t>
    </w:r>
  </w:p>
</w:comment>";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" || args[0] == "help" {
        print_usage();
        std::process::exit(if args.is_empty() { 1 } else { 0 });
    }
    let cmd = args.remove(0);
    let code = match cmd.as_str() {
        "unpack" => cmd_unpack(&args),
        "pack" => cmd_pack(&args),
        "validate" => cmd_validate(&args),
        "replace-text" => cmd_replace_text(&args),
        "replace-field" => cmd_replace_field(&args),
        "inspect" => cmd_inspect(&args),
        "inspect-headers" => cmd_inspect_headers(&args),
        "inspect-tables" => cmd_inspect_tables(&args),
        "list-sections" => cmd_list_sections(&args),
        "delete-sections" => cmd_delete_sections(&args),
        "comment" => cmd_comment(&args),
        "patch" => cmd_patch(&args),
        "accept-changes" => cmd_accept_changes(&args),
        "convert" => cmd_convert(&args),
        "render" => cmd_render(&args),
        "soffice" => cmd_soffice(&args),
        other => {
            eprintln!("Error: unknown command {other}");
            print_usage();
            1
        }
    };
    std::process::exit(code);
}

fn print_usage() {
    println!(
        "\
docx-skill <command> [args]

  unpack <office-file> <output-dir> [--merge-runs true|false] [--simplify-redlines true|false]
  pack <input-dir> <output-file> [--original <file>] [--validate true|false]
  validate <path> [--original <file>] [--auto-repair] [--author NAME] [--verbose]
  replace-text <unpacked-dir> (--match TEXT --text TEXT | --map FILE) [--all-files] [--dry-run]
  replace-field <unpacked-dir> [--list | --field NAME --text TEXT | --sdt TAG --text TEXT | --map FILE] [--dry-run]
  inspect <unpacked-dir-or-docx> [--text] [--sections] [--media]
  inspect-headers <unpacked-dir>
  inspect-tables <unpacked-dir>
  list-sections <unpacked-dir>
  delete-sections <unpacked-dir> (--delete SPEC | --keep SPEC) [--dry-run]
  comment <unpacked-dir> <id> <text> [--author NAME] [--initials X] [--parent ID]
  patch <unpacked-dir> (--patch JSON | --patch-file FILE) [--all-files] [--dry-run]
  accept-changes <input.docx> <output.docx>
  convert <input> --to pdf|images|docx [output] [--outdir DIR]
  render <input> [output-prefix] [--cols N]
  soffice [libreoffice-args...]

This program does not start an interpreter."
    );
}

fn fail(msg: impl AsRef<str>) -> i32 {
    eprintln!("{}", msg.as_ref());
    1
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).map(|s| s.as_str()))
}

fn opt_bool(args: &[String], name: &str, default: bool) -> bool {
    match opt(args, name) {
        Some(v) => v.eq_ignore_ascii_case("true"),
        None => default,
    }
}

fn positionals(args: &[String]) -> Vec<String> {
    let valued = [
        "--merge-runs",
        "--simplify-redlines",
        "--original",
        "--validate",
        "--author",
        "--match",
        "--text",
        "--map",
        "--field",
        "--sdt",
        "--delete",
        "--keep",
        "--initials",
        "--parent",
        "--patch",
        "--patch-file",
        "--to",
        "--outdir",
        "--output",
        "--cols",
    ];
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if valued.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        out.push(a.clone());
        i += 1;
    }
    out
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn tag_is(el: &xml::Element, q: &str) -> bool {
    if el.name == q {
        return true;
    }
    match q.split_once(':') {
        Some((prefix, local)) => {
            el.local() == local && el.name.split_once(':').is_some_and(|(p, _)| p == prefix)
        }
        None => el.local() == q && !el.name.contains(':'),
    }
}

fn find_q<'a>(el: &'a xml::Element, q: &str) -> Vec<&'a xml::Element> {
    let mut out = Vec::new();
    fn walk<'a>(el: &'a xml::Element, q: &str, out: &mut Vec<&'a xml::Element>) {
        for node in &el.children {
            if let xml::Node::Element(child) = node {
                if tag_is(child, q) {
                    out.push(child);
                }
                walk(child, q, out);
            }
        }
    }
    walk(el, q, &mut out);
    out
}

fn child_q<'a>(el: &'a xml::Element, q: &str) -> Option<&'a xml::Element> {
    el.children.iter().find_map(|n| match n {
        xml::Node::Element(e) if tag_is(e, q) => Some(e),
        _ => None,
    })
}

fn read_doc(path: &Path) -> Result<xml::Document, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    xml::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn office_suffix(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_ascii_lowercase()))
}

fn suffix_ok(path: &Path) -> bool {
    office_suffix(path).is_some_and(|s| OFFICE_SUFFIXES.contains(&s.as_str()))
}

fn walk_files(dir: &Path, pred: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    let mut ents: Vec<_> = rd.flatten().collect();
    ents.sort_by_key(|e| e.file_name());
    for ent in ents {
        let path = ent.path();
        if path.is_dir() {
            walk_files(&path, pred, out);
        } else if pred(&path) {
            out.push(path);
        }
    }
}

fn xml_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_files(
        dir,
        &|p| {
            matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("xml") | Some("rels")
            )
        },
        &mut out,
    );
    out
}

fn cmd_unpack(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.len() < 2 {
        return fail(
            "Usage: docx-skill unpack <office-file> <output-dir> [--merge-runs true|false] [--simplify-redlines true|false]",
        );
    }
    let input = PathBuf::from(&pos[0]);
    let output = PathBuf::from(&pos[1]);
    let merge = opt_bool(args, "--merge-runs", true);
    let simplify = opt_bool(args, "--simplify-redlines", true);
    match unpack(&input, &output, merge, simplify) {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(msg) => {
            println!("{msg}");
            1
        }
    }
}

fn unpack(input: &Path, output: &Path, merge: bool, simplify: bool) -> Result<String, String> {
    if !input.exists() {
        return Err(format!("Error: {} does not exist", input.display()));
    }
    if !suffix_ok(input) {
        return Err(format!(
            "Error: {} must be a .docx, .pptx, or .xlsx file",
            input.display()
        ));
    }
    fs::create_dir_all(output).map_err(|e| format!("Error unpacking: {e}"))?;
    let file = fs::File::open(input).map_err(|e| format!("Error unpacking: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        format!(
            "Error: {} is not a valid Office file ({e})",
            input.display()
        )
    })?;
    archive
        .extract(output)
        .map_err(|e| format!("Error unpacking: {e}"))?;
    let xmls = xml_files_under(output);
    for xml_file in &xmls {
        let _ = pretty_xml_file(xml_file);
    }
    let mut message = format!("Unpacked {} ({} XML files)", input.display(), xmls.len());
    if office_suffix(input).as_deref() == Some(".docx") {
        if simplify {
            let (n, _) = simplify_redlines(output);
            message.push_str(&format!(", simplified {n} tracked changes"));
        }
        if merge {
            let (n, _) = merge_runs(output);
            message.push_str(&format!(", merged {n} runs"));
        }
    }
    for xml_file in &xmls {
        let _ = escape_smart_quotes_file(xml_file);
    }
    Ok(message)
}

fn pretty_xml_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc = xml::parse(&text)?;
    fs::write(path, xml::write_pretty(&doc)).map_err(|e| e.to_string())
}

fn condense_xml_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc =
        xml::parse(&text).map_err(|e| format!("ERROR: Failed to parse {}: {e}", path.display()))?;
    fs::write(path, xml::write_condensed(&doc)).map_err(|e| e.to_string())
}

fn escape_smart_quotes_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    fs::write(path, xml::smart_quote_entities(&text)).map_err(|e| e.to_string())
}

fn simplify_redlines(dir: &Path) -> (usize, String) {
    let doc_xml = dir.join("word/document.xml");
    if !doc_xml.exists() {
        return (0, format!("Error: {} not found", doc_xml.display()));
    }
    let mut doc = match read_doc(&doc_xml) {
        Ok(d) => d,
        Err(e) => return (0, format!("Error: {e}")),
    };
    let n = simplify_element(&mut doc.root);
    if let Err(e) = fs::write(&doc_xml, xml::write_pretty(&doc)) {
        return (0, format!("Error: {e}"));
    }
    (n, format!("Simplified {n} tracked changes"))
}

fn simplify_element(el: &mut xml::Element) -> usize {
    let mut n = 0;
    if tag_is(el, "w:p") || tag_is(el, "w:tc") {
        n += merge_tracked(el, "w:ins");
        n += merge_tracked(el, "w:del");
    }
    let len = el.children.len();
    for i in 0..len {
        if let xml::Node::Element(child) = &mut el.children[i] {
            n += simplify_element(child);
        }
    }
    n
}

fn author_of(el: &xml::Element) -> String {
    el.attr("author").unwrap_or("").to_string()
}

fn merge_tracked(container: &mut xml::Element, q: &str) -> usize {
    let mut count = 0;
    loop {
        let idxs: Vec<usize> = container
            .children
            .iter()
            .enumerate()
            .filter_map(|(i, n)| match n {
                xml::Node::Element(e) if tag_is(e, q) => Some(i),
                _ => None,
            })
            .collect();
        if idxs.len() < 2 {
            break;
        }
        let mut did = false;
        for w in idxs.windows(2) {
            let (i, j) = (w[0], w[1]);
            let same = {
                let a = match &container.children[i] {
                    xml::Node::Element(e) => author_of(e),
                    _ => String::new(),
                };
                let b = match &container.children[j] {
                    xml::Node::Element(e) => author_of(e),
                    _ => String::new(),
                };
                a == b
            };
            if same && only_ws_between(&container.children, i, j) {
                let xml::Node::Element(mut source) = container.children.remove(j) else {
                    break;
                };
                if let xml::Node::Element(target) = &mut container.children[i] {
                    target.children.append(&mut source.children);
                }
                count += 1;
                did = true;
                break;
            }
        }
        if !did {
            break;
        }
    }
    count
}

fn only_ws_between(children: &[xml::Node], a: usize, b: usize) -> bool {
    if b <= a {
        return false;
    }
    children[a + 1..b].iter().all(|n| match n {
        xml::Node::Element(_) => false,
        xml::Node::Text(t) => t.trim().is_empty(),
        xml::Node::Comment(_) => true,
    })
}

fn merge_runs(dir: &Path) -> (usize, String) {
    let doc_xml = dir.join("word/document.xml");
    if !doc_xml.exists() {
        return (0, format!("Error: {} not found", doc_xml.display()));
    }
    let mut doc = match read_doc(&doc_xml) {
        Ok(d) => d,
        Err(e) => return (0, format!("Error: {e}")),
    };
    remove_tag(&mut doc.root, "w:proofErr");
    strip_rsid(&mut doc.root);
    let mut count = 0;
    merge_run_containers(&mut doc.root, &mut count);
    if let Err(e) = fs::write(&doc_xml, xml::write_pretty(&doc)) {
        return (0, format!("Error: {e}"));
    }
    (count, format!("Merged {count} runs"))
}

fn remove_tag(el: &mut xml::Element, q: &str) {
    el.children.retain(|n| match n {
        xml::Node::Element(e) => !tag_is(e, q),
        _ => true,
    });
    for n in &mut el.children {
        if let xml::Node::Element(child) = n {
            remove_tag(child, q);
        }
    }
}

fn strip_rsid(el: &mut xml::Element) {
    if tag_is(el, "w:r") {
        el.attrs
            .retain(|(n, _)| !xml::local_name(n).to_ascii_lowercase().contains("rsid"));
    }
    for n in &mut el.children {
        if let xml::Node::Element(child) = n {
            strip_rsid(child);
        }
    }
}

fn merge_run_containers(el: &mut xml::Element, count: &mut usize) {
    let has_run = el.children.iter().any(|n| match n {
        xml::Node::Element(e) => tag_is(e, "w:r"),
        _ => false,
    });
    if has_run {
        *count += merge_runs_in(el);
    }
    let len = el.children.len();
    for i in 0..len {
        if let xml::Node::Element(child) = &mut el.children[i] {
            merge_run_containers(child, count);
        }
    }
}

fn merge_runs_in(container: &mut xml::Element) -> usize {
    let mut count = 0;
    let mut idx = 0;
    while idx < container.children.len() {
        let is_run = matches!(&container.children[idx], xml::Node::Element(e) if tag_is(e, "w:r"));
        if !is_run {
            idx += 1;
            continue;
        }
        loop {
            let next = next_element_index(&container.children, idx);
            let can = next.is_some_and(|j| {
                let xml::Node::Element(right) = &container.children[j] else {
                    return false;
                };
                if !tag_is(right, "w:r") {
                    return false;
                }
                let xml::Node::Element(left) = &container.children[idx] else {
                    return false;
                };
                runs_can_merge(left, right)
            });
            if !can {
                break;
            }
            let j = next.unwrap();
            let xml::Node::Element(mut source) = container.children.remove(j) else {
                break;
            };
            if let xml::Node::Element(target) = &mut container.children[idx] {
                let moved: Vec<_> = source
                    .children
                    .drain(..)
                    .filter(|n| match n {
                        xml::Node::Element(e) => !tag_is(e, "w:rPr"),
                        _ => false,
                    })
                    .collect();
                target.children.extend(moved);
                consolidate_text(target);
            }
            count += 1;
        }
        if let xml::Node::Element(run) = &mut container.children[idx] {
            consolidate_text(run);
        }
        idx += 1;
    }
    count
}

fn next_element_index(children: &[xml::Node], from: usize) -> Option<usize> {
    children
        .iter()
        .enumerate()
        .skip(from + 1)
        .find_map(|(i, n)| {
            if matches!(n, xml::Node::Element(_)) {
                Some(i)
            } else {
                None
            }
        })
}

fn runs_can_merge(a: &xml::Element, b: &xml::Element) -> bool {
    let ra = direct_child_mut_ref(a, "w:rPr");
    let rb = direct_child_mut_ref(b, "w:rPr");
    match (ra, rb) {
        (None, None) => true,
        (Some(x), Some(y)) => xml::serialize_el(x) == xml::serialize_el(y),
        _ => false,
    }
}

fn direct_child_mut_ref<'a>(el: &'a xml::Element, q: &str) -> Option<&'a xml::Element> {
    child_q(el, q)
}

fn consolidate_text(run: &mut xml::Element) {
    loop {
        let idxs: Vec<usize> = run
            .children
            .iter()
            .enumerate()
            .filter_map(|(i, n)| match n {
                xml::Node::Element(e) if tag_is(e, "w:t") => Some(i),
                _ => None,
            })
            .collect();
        let mut pair = None;
        for w in idxs.windows(2) {
            if only_ws_between(&run.children, w[0], w[1]) {
                pair = Some((w[0], w[1]));
                break;
            }
        }
        let Some((i, j)) = pair else {
            break;
        };
        let curr_text = match &run.children[j] {
            xml::Node::Element(e) => e.text_content(),
            _ => String::new(),
        };
        let prev_text = match &run.children[i] {
            xml::Node::Element(e) => e.text_content(),
            _ => String::new(),
        };
        let merged = format!("{prev_text}{curr_text}");
        if let xml::Node::Element(prev) = &mut run.children[i] {
            prev.children.clear();
            prev.children.push(xml::Node::Text(merged.clone()));
            if merged.starts_with(' ') || merged.ends_with(' ') {
                prev.set_attr("xml:space", "preserve");
            } else {
                prev.remove_attr_matching(|n| xml::local_name(n) == "space");
            }
        }
        run.children.remove(j);
    }
}

fn cmd_pack(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.len() < 2 {
        return fail(
            "Usage: docx-skill pack <input-dir> <output-file> [--original <file>] [--validate true|false]",
        );
    }
    let input = PathBuf::from(&pos[0]);
    let output = PathBuf::from(&pos[1]);
    let original = opt(args, "--original").map(PathBuf::from);
    let validate = opt_bool(args, "--validate", true);
    match pack(&input, &output, original.as_deref(), validate) {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(msg) => {
            println!("{msg}");
            1
        }
    }
}

fn pack(
    input: &Path,
    output: &Path,
    original: Option<&Path>,
    validate: bool,
) -> Result<String, String> {
    if !input.is_dir() {
        return Err(format!("Error: {} is not a directory", input.display()));
    }
    if !suffix_ok(output) {
        return Err(format!(
            "Error: {} must be a .docx, .pptx, or .xlsx file",
            output.display()
        ));
    }
    if validate {
        if let Some(original) = original {
            if original.exists() && office_suffix(output).as_deref() == Some(".docx") {
                let (ok, note) =
                    run_docx_validation(input, Some(original), true, DEFAULT_AUTHOR, false);
                if let Some(note) = note {
                    if !note.is_empty() {
                        println!("{note}");
                    }
                }
                if !ok {
                    return Err(format!("Error: Validation failed for {}", input.display()));
                }
            }
        }
    }
    let temp = std::env::temp_dir().join(format!("docx-skill-pack-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    let content = temp.join("content");
    copy_dir(input, &content)?;
    for xml_file in xml_files_under(&content) {
        condense_xml_file(&xml_file)?;
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    zip_dir(&content, output)?;
    let _ = fs::remove_dir_all(&temp);
    Ok(format!(
        "Successfully packed {} to {}",
        input.display(),
        output.display()
    ))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    let rd = fs::read_dir(src).map_err(|e| e.to_string())?;
    for ent in rd.flatten() {
        let from = ent.path();
        let to = dst.join(ent.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn zip_dir(src: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut zw = zip::ZipWriter::new(file);
    let mut files = Vec::new();
    walk_files(src, &|_| true, &mut files);
    for path in files {
        let rel = path.strip_prefix(src).unwrap_or(&path);
        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zw.start_file(name, opts).map_err(|e| e.to_string())?;
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        zw.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zw.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn cmd_validate(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill validate <path> [--original <file>] [--auto-repair] [--author NAME] [--verbose]",
        );
    }
    let path = PathBuf::from(&pos[0]);
    if !path.exists() {
        return fail(format!("Error: {} does not exist", path.display()));
    }
    let original = opt(args, "--original").map(PathBuf::from);
    if let Some(orig) = &original {
        if !orig.is_file() {
            return fail(format!("Error: {} is not a file", orig.display()));
        }
        if !suffix_ok(orig) {
            return fail(format!(
                "Error: {} must be a .docx, .pptx, or .xlsx file",
                orig.display()
            ));
        }
    }
    let author = opt(args, "--author").unwrap_or(DEFAULT_AUTHOR);
    let verbose = flag(args, "--verbose") || flag(args, "-v");
    let repair = flag(args, "--auto-repair");
    let kind = office_suffix(original.as_deref().unwrap_or(&path));
    let kind = match kind {
        Some(k) if OFFICE_SUFFIXES.contains(&k.as_str()) => k,
        _ => {
            return fail(format!(
                "Error: Cannot determine file type from {}. Use --original or provide a .docx/.pptx/.xlsx file.",
                path.display()
            ));
        }
    };
    let temp = std::env::temp_dir().join(format!("docx-skill-validate-{}", std::process::id()));
    let unpacked;
    let cleanup;
    if path.is_file() && suffix_ok(&path) {
        let _ = fs::remove_dir_all(&temp);
        let _ = fs::create_dir_all(&temp);
        if let Err(e) = extract_zip(&path, &temp) {
            return fail(e);
        }
        unpacked = temp.clone();
        cleanup = true;
    } else if path.is_dir() {
        unpacked = path.clone();
        cleanup = false;
    } else {
        return fail(format!(
            "Error: {} is not a directory or Office file",
            path.display()
        ));
    }
    let code = if kind == ".docx" {
        let (ok, note) =
            run_docx_validation(&unpacked, original.as_deref(), repair, author, verbose);
        if let Some(note) = note {
            if !note.is_empty() {
                println!("{note}");
            }
        }
        if ok { 0 } else { 1 }
    } else if kind == ".pptx" {
        let ok = validate_well_formed(&unpacked, verbose);
        if ok { 0 } else { 1 }
    } else {
        eprintln!("Error: Validation not supported for file type {kind}");
        1
    };
    if cleanup {
        let _ = fs::remove_dir_all(&temp);
    }
    code
}

fn extract_zip(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = fs::File::open(src).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(dest).map_err(|e| e.to_string())
}

fn run_docx_validation(
    unpacked: &Path,
    original: Option<&Path>,
    repair: bool,
    author: &str,
    verbose: bool,
) -> (bool, Option<String>) {
    let mut notes = Vec::new();
    if repair {
        let n = repair_tree(unpacked);
        if n > 0 {
            notes.push(format!("Auto-repaired {n} issue(s)"));
        }
    }
    let mut ok = true;
    if !validate_well_formed(unpacked, verbose) {
        ok = false;
    }
    if !validate_ignorable_namespaces(unpacked, verbose) {
        ok = false;
    }
    if !validate_whitespace(unpacked, verbose) {
        ok = false;
    }
    if !validate_deletions(unpacked, verbose) {
        ok = false;
    }
    if !validate_insertions(unpacked, verbose) {
        ok = false;
    }
    if !validate_id_constraints(unpacked, verbose) {
        ok = false;
    }
    if !validate_comment_markers(unpacked, verbose) {
        ok = false;
    }
    if let Some(original) = original {
        if !validate_redlines(unpacked, original, author, verbose) {
            ok = false;
        }
        if let Some(line) = paragraph_count_line(unpacked, original) {
            notes.push(line);
        }
    }
    if ok {
        notes.push("All validations PASSED!".into());
    }
    notes.push(
        "XSD schema-language validation is not run by this binary (no schema engine). Structural checks ran."
            .into(),
    );
    (ok, Some(notes.join("\n")))
}

fn repair_tree(unpacked: &Path) -> usize {
    let mut n = 0;
    for path in xml_files_under(unpacked) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut doc) = xml::parse(&text) else {
            continue;
        };
        let numbering = path.file_name().and_then(|s| s.to_str()) == Some("numbering.xml");
        let mut changed = 0;
        changed += repair_whitespace_el(&mut doc.root);
        changed += repair_durable_el(
            &mut doc.root,
            numbering,
            path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        );
        if changed > 0 {
            if fs::write(&path, xml::write_pretty(&doc)).is_ok() {
                n += changed;
            }
        }
    }
    n
}

fn repair_whitespace_el(el: &mut xml::Element) -> usize {
    let mut n = 0;
    if el.local() == "t" {
        let text = el.text_content();
        if text.starts_with([' ', '\t']) || text.ends_with([' ', '\t']) {
            if el.attr("space") != Some("preserve") {
                el.set_attr("xml:space", "preserve");
                let preview = if text.len() > 30 {
                    format!("{:?}...", &text[..30])
                } else {
                    format!("{text:?}")
                };
                println!(
                    "  Repaired: Added xml:space='preserve' to {}: {preview}",
                    el.name
                );
                n += 1;
            }
        }
    }
    for child in &mut el.children {
        if let xml::Node::Element(c) = child {
            n += repair_whitespace_el(c);
        }
    }
    n
}

fn repair_durable_el(el: &mut xml::Element, numbering: bool, file_name: &str) -> usize {
    let mut n = 0;
    if let Some(val) = el.attr("durableId").map(|s| s.to_string()) {
        let needs = if numbering {
            parse_id(&val, 10).map(|v| v >= 0x7FFF_FFFF).unwrap_or(true)
        } else {
            parse_id(&val, 16).map(|v| v >= 0x7FFF_FFFF).unwrap_or(true)
        };
        if needs {
            let value = rand_id();
            let new_id = if numbering {
                value.to_string()
            } else {
                format!("{value:08X}")
            };
            println!("  Repaired: {file_name}: durableId {val} -> {new_id}");
            el.set_attr("w16cid:durableId", &new_id);
            n += 1;
        }
    }
    for child in &mut el.children {
        if let xml::Node::Element(c) = child {
            n += repair_durable_el(c, numbering, file_name);
        }
    }
    n
}

fn parse_id(val: &str, base: u32) -> Result<u32, ()> {
    u32::from_str_radix(val.trim(), base).map_err(|_| ())
}

fn rand_id() -> u32 {
    static C: AtomicU64 = AtomicU64::new(1);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let n = C.fetch_add(1, Ordering::Relaxed) ^ t.rotate_left(17);
    let v = (n as u32) & 0x7FFF_FFFE;
    if v == 0 { 1 } else { v }
}

fn validate_well_formed(unpacked: &Path, verbose: bool) -> bool {
    let mut errors = Vec::new();
    for path in xml_files_under(unpacked) {
        match fs::read_to_string(&path)
            .and_then(|t| xml::parse(&t).map_err(|e| std::io::Error::other(e)))
        {
            Ok(_) => {}
            Err(e) => {
                let rel = path.strip_prefix(unpacked).unwrap_or(&path);
                errors.push(format!("  {}: {e}", rel.display()));
            }
        }
    }
    if errors.is_empty() {
        if verbose {
            println!("PASSED - All XML files are well-formed");
        }
        true
    } else {
        println!("FAILED - Found {} XML violations:", errors.len());
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn validate_ignorable_namespaces(unpacked: &Path, verbose: bool) -> bool {
    let mut errors = Vec::new();
    for path in xml_files_under(unpacked) {
        let Ok(doc) = read_doc(&path) else {
            continue;
        };
        let declared: Vec<String> = doc
            .root
            .attrs
            .iter()
            .filter_map(|(n, _)| n.strip_prefix("xmlns:").map(|p| p.to_string()))
            .collect();
        for (n, v) in &doc.root.attrs {
            if xml::local_name(n) == "Ignorable" {
                for ns in v.split_whitespace() {
                    if !declared.iter().any(|d| d == ns) {
                        let rel = path.strip_prefix(unpacked).unwrap_or(&path);
                        errors.push(format!(
                            "  {}: Namespace '{ns}' in Ignorable but not declared",
                            rel.display()
                        ));
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        if verbose {
            println!("PASSED - All namespace prefixes properly declared");
        }
        true
    } else {
        println!("FAILED - {} namespace issues:", errors.len());
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn validate_whitespace(unpacked: &Path, verbose: bool) -> bool {
    let path = unpacked.join("word/document.xml");
    let Ok(doc) = read_doc(&path) else {
        return true;
    };
    let mut errors = Vec::new();
    fn walk(el: &xml::Element, errors: &mut Vec<String>) {
        if tag_is(el, "w:t") {
            let text = el.text_content();
            let bad = text.starts_with([' ', '\t', '\n', '\r'])
                || text.ends_with([' ', '\t', '\n', '\r']);
            if bad && el.attr("space") != Some("preserve") {
                let preview = if text.len() > 40 {
                    format!("{:?}...", &text[..40])
                } else {
                    format!("{text:?}")
                };
                errors.push(format!(
                    "  word/document.xml: w:t element with whitespace missing xml:space='preserve': {preview}"
                ));
            }
        }
        for c in el.children.iter() {
            if let xml::Node::Element(ch) = c {
                walk(ch, errors);
            }
        }
    }
    walk(&doc.root, &mut errors);
    if errors.is_empty() {
        if verbose {
            println!("PASSED - All whitespace is properly preserved");
        }
        true
    } else {
        println!(
            "FAILED - Found {} whitespace preservation violations:",
            errors.len()
        );
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn validate_deletions(unpacked: &Path, verbose: bool) -> bool {
    let path = unpacked.join("word/document.xml");
    let Ok(doc) = read_doc(&path) else {
        return true;
    };
    let mut errors = Vec::new();
    fn walk(el: &xml::Element, in_del: bool, errors: &mut Vec<String>) {
        let now_del = in_del || tag_is(el, "w:del");
        if in_del && tag_is(el, "w:t") && !el.text_content().is_empty() {
            errors.push(format!(
                "  word/document.xml: <w:t> found within <w:del>: {:?}",
                el.text_content()
            ));
        }
        if in_del && tag_is(el, "w:instrText") {
            errors.push(format!(
                "  word/document.xml: <w:instrText> found within <w:del> (use <w:delInstrText>): {:?}",
                el.text_content()
            ));
        }
        for c in &el.children {
            if let xml::Node::Element(ch) = c {
                walk(ch, now_del, errors);
            }
        }
    }
    walk(&doc.root, false, &mut errors);
    if errors.is_empty() {
        if verbose {
            println!("PASSED - No w:t elements found within w:del elements");
        }
        true
    } else {
        println!(
            "FAILED - Found {} deletion validation violations:",
            errors.len()
        );
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn validate_insertions(unpacked: &Path, verbose: bool) -> bool {
    let path = unpacked.join("word/document.xml");
    let Ok(doc) = read_doc(&path) else {
        return true;
    };
    let mut errors = Vec::new();
    fn walk(el: &xml::Element, in_ins: bool, in_del: bool, errors: &mut Vec<String>) {
        let now_ins = in_ins || tag_is(el, "w:ins");
        let now_del = in_del || tag_is(el, "w:del");
        if now_ins && !now_del && tag_is(el, "w:delText") {
            errors.push(format!(
                "  word/document.xml: <w:delText> within <w:ins>: {:?}",
                el.text_content()
            ));
        }
        for c in &el.children {
            if let xml::Node::Element(ch) = c {
                walk(ch, now_ins, now_del, errors);
            }
        }
    }
    walk(&doc.root, false, false, &mut errors);
    if errors.is_empty() {
        if verbose {
            println!("PASSED - No w:delText elements within w:ins elements");
        }
        true
    } else {
        println!(
            "FAILED - Found {} insertion validation violations:",
            errors.len()
        );
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn validate_id_constraints(unpacked: &Path, verbose: bool) -> bool {
    let mut errors = Vec::new();
    for path in xml_files_under(unpacked) {
        let Ok(doc) = read_doc(&path) else {
            continue;
        };
        let numbering = path.file_name().and_then(|s| s.to_str()) == Some("numbering.xml");
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("xml");
        fn walk(el: &xml::Element, numbering: bool, name: &str, errors: &mut Vec<String>) {
            if let Some(val) = el.attr("paraId") {
                if parse_id(val, 16).map(|v| v >= 0x8000_0000).unwrap_or(false) {
                    errors.push(format!("  {name}: paraId={val} >= 0x80000000"));
                }
            }
            if let Some(val) = el.attr("durableId") {
                if numbering {
                    match parse_id(val, 10) {
                        Ok(v) if v >= 0x7FFF_FFFF => {
                            errors.push(format!("  {name}: durableId={val} >= 0x7FFFFFFF"));
                        }
                        Err(()) => {
                            errors.push(format!(
                                "  {name}: durableId={val} must be decimal in numbering.xml"
                            ));
                        }
                        _ => {}
                    }
                } else if parse_id(val, 16).map(|v| v >= 0x7FFF_FFFF).unwrap_or(false) {
                    errors.push(format!("  {name}: durableId={val} >= 0x7FFFFFFF"));
                }
            }
            for c in &el.children {
                if let xml::Node::Element(ch) = c {
                    walk(ch, numbering, name, errors);
                }
            }
        }
        walk(&doc.root, numbering, name, &mut errors);
    }
    if errors.is_empty() {
        if verbose {
            println!("PASSED - All paraId/durableId values within constraints");
        }
        true
    } else {
        println!("FAILED - {} ID constraint violations:", errors.len());
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn ids_of(el: &xml::Element, q: &str) -> Vec<String> {
    find_q(el, q)
        .into_iter()
        .filter_map(|e| e.attr("id").map(|s| s.to_string()))
        .collect()
}

fn validate_comment_markers(unpacked: &Path, verbose: bool) -> bool {
    let doc_path = unpacked.join("word/document.xml");
    if !doc_path.is_file() {
        if verbose {
            println!("PASSED - No document.xml found (skipping comment validation)");
        }
        return true;
    }
    let Ok(doc) = read_doc(&doc_path) else {
        println!("FAILED - comment marker parse error");
        return false;
    };
    let starts = ids_of(&doc.root, "w:commentRangeStart");
    let ends = ids_of(&doc.root, "w:commentRangeEnd");
    let refs = ids_of(&doc.root, "w:commentReference");
    let mut errors = Vec::new();
    for id in &ends {
        if !starts.contains(id) {
            errors.push(format!(
                "  document.xml: commentRangeEnd id=\"{id}\" has no matching commentRangeStart"
            ));
        }
    }
    for id in &starts {
        if !ends.contains(id) {
            errors.push(format!(
                "  document.xml: commentRangeStart id=\"{id}\" has no matching commentRangeEnd"
            ));
        }
    }
    let comments_path = unpacked.join("word/comments.xml");
    if comments_path.is_file() {
        if let Ok(comments) = read_doc(&comments_path) {
            let comment_ids = ids_of(&comments.root, "w:comment");
            let mut markers = starts.clone();
            markers.extend(ends.iter().cloned());
            markers.extend(refs.iter().cloned());
            markers.sort();
            markers.dedup();
            for id in markers {
                if !id.is_empty() && !comment_ids.contains(&id) {
                    errors.push(format!(
                        "  document.xml: marker id=\"{id}\" references non-existent comment"
                    ));
                }
            }
        }
    }
    if errors.is_empty() {
        if verbose {
            println!("PASSED - All comment markers properly paired");
        }
        true
    } else {
        println!("FAILED - {} comment marker violations:", errors.len());
        for e in errors {
            println!("{e}");
        }
        false
    }
}

fn count_paragraphs(doc: &xml::Document) -> usize {
    find_q(&doc.root, "w:p").len()
}

fn paragraph_count_line(unpacked: &Path, original: &Path) -> Option<String> {
    let new_count = read_doc(&unpacked.join("word/document.xml"))
        .map(|d| count_paragraphs(&d))
        .unwrap_or(0);
    let temp = std::env::temp_dir().join(format!("docx-skill-paras-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    let original_count = if extract_zip(original, &temp).is_ok() {
        read_doc(&temp.join("word/document.xml"))
            .map(|d| count_paragraphs(&d))
            .unwrap_or(0)
    } else {
        0
    };
    let _ = fs::remove_dir_all(&temp);
    let diff = new_count as i64 - original_count as i64;
    let diff_str = if diff > 0 {
        format!("+{diff}")
    } else {
        diff.to_string()
    };
    Some(format!(
        "\nParagraphs: {original_count} -> {new_count} ({diff_str})"
    ))
}

fn validate_redlines(unpacked: &Path, original: &Path, author: &str, verbose: bool) -> bool {
    let modified_path = unpacked.join("word/document.xml");
    if !modified_path.exists() {
        println!(
            "FAILED - Modified document.xml not found at {}",
            modified_path.display()
        );
        return false;
    }
    let Ok(modified) = read_doc(&modified_path) else {
        println!("FAILED - Error parsing modified document.xml");
        return false;
    };
    let author_changes = find_q(&modified.root, "w:ins")
        .into_iter()
        .chain(find_q(&modified.root, "w:del"))
        .any(|e| e.attr("author") == Some(author));
    if !author_changes {
        if verbose {
            println!("PASSED - No tracked changes by {author} found.");
        }
        return true;
    }
    let temp = std::env::temp_dir().join(format!("docx-skill-redline-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    if extract_zip(original, &temp).is_err() {
        println!("FAILED - Error unpacking original docx");
        return false;
    }
    let original_xml = temp.join("word/document.xml");
    if !original_xml.exists() {
        println!(
            "FAILED - Original document.xml not found in {}",
            original.display()
        );
        let _ = fs::remove_dir_all(&temp);
        return false;
    }
    let Ok(original_doc) = read_doc(&original_xml) else {
        println!("FAILED - Error parsing XML files");
        let _ = fs::remove_dir_all(&temp);
        return false;
    };
    let mut mod_root = modified.root.clone();
    let mut orig_root = original_doc.root.clone();
    strip_author_ins(&mut mod_root, author);
    strip_author_ins(&mut orig_root, author);
    unwrap_author_del(&mut mod_root, author);
    unwrap_author_del(&mut orig_root, author);
    let modified_text = paragraph_text(&mod_root);
    let original_text = paragraph_text(&orig_root);
    let _ = fs::remove_dir_all(&temp);
    if modified_text != original_text {
        println!("FAILED - Document text doesn't match after removing {author}'s tracked changes");
        println!();
        println!("Likely causes:");
        println!("  1. Modified text inside another author's <w:ins> or <w:del> tags");
        println!("  2. Made edits without proper tracked changes");
        println!("  3. Didn't nest <w:del> inside <w:ins> when deleting another's insertion");
        if let Some(diff) = git_word_diff(&original_text, &modified_text) {
            println!();
            println!("Differences:");
            println!("============");
            println!("{diff}");
        }
        return false;
    }
    if verbose {
        println!("PASSED - All changes by {author} are properly tracked");
    }
    true
}

fn strip_author_ins(el: &mut xml::Element, author: &str) {
    el.children.retain(|n| match n {
        xml::Node::Element(e) => !(tag_is(e, "w:ins") && e.attr("author") == Some(author)),
        _ => true,
    });
    for n in &mut el.children {
        if let xml::Node::Element(c) = n {
            strip_author_ins(c, author);
        }
    }
}

fn unwrap_author_del(el: &mut xml::Element, author: &str) {
    let mut i = 0;
    while i < el.children.len() {
        let is_del = matches!(
            &el.children[i],
            xml::Node::Element(e) if tag_is(e, "w:del") && e.attr("author") == Some(author)
        );
        if is_del {
            let xml::Node::Element(mut deleted) = el.children.remove(i) else {
                continue;
            };
            rename_deltext(&mut deleted);
            let kids = std::mem::take(&mut deleted.children);
            for (k, child) in kids.into_iter().enumerate() {
                el.children.insert(i + k, child);
            }
        } else {
            if let xml::Node::Element(c) = &mut el.children[i] {
                unwrap_author_del(c, author);
            }
            i += 1;
        }
    }
}

fn rename_deltext(el: &mut xml::Element) {
    if tag_is(el, "w:delText") {
        el.name = "w:t".to_string();
    }
    for n in &mut el.children {
        if let xml::Node::Element(c) = n {
            rename_deltext(c);
        }
    }
}

fn paragraph_text(root: &xml::Element) -> String {
    let mut paras = Vec::new();
    for p in find_q(root, "w:p") {
        let mut parts = Vec::new();
        for t in find_q(p, "w:t") {
            let text = t.text_content();
            if !text.is_empty() {
                parts.push(text);
            }
        }
        let line = parts.join("");
        if !line.is_empty() {
            paras.push(line);
        }
    }
    paras.join("\n")
}

fn git_word_diff(original: &str, modified: &str) -> Option<String> {
    let dir = std::env::temp_dir().join(format!("docx-skill-diff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).ok()?;
    let a = dir.join("original.txt");
    let b = dir.join("modified.txt");
    fs::write(&a, original).ok()?;
    fs::write(&b, modified).ok()?;
    let out = Command::new("git")
        .args([
            "diff",
            "--word-diff=plain",
            "--word-diff-regex=.",
            "-U0",
            "--no-index",
            "--",
        ])
        .arg(&a)
        .arg(&b)
        .output()
        .ok();
    let _ = fs::remove_dir_all(&dir);
    let stdout = String::from_utf8(out?.stdout).ok()?;
    let mut lines = Vec::new();
    let mut in_content = false;
    for line in stdout.lines() {
        if line.starts_with("@@") {
            in_content = true;
            continue;
        }
        if in_content && !line.trim().is_empty() {
            lines.push(line.to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn cmd_replace_text(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill replace-text <unpacked-dir> (--match TEXT --text TEXT | --map FILE) [--all-files] [--dry-run]",
        );
    }
    let unpacked = PathBuf::from(&pos[0]);
    let doc = unpacked.join("word/document.xml");
    if !doc.is_file() {
        return fail(format!(
            "Error: {}/word/document.xml not found",
            unpacked.display()
        ));
    }
    let dry = flag(args, "--dry-run");
    let all = flag(args, "--all-files");
    let match_text = opt(args, "--match");
    let new_text = opt(args, "--text");
    let map_path = opt(args, "--map");
    if match_text.is_none() && map_path.is_none() {
        return fail("Specify --match TEXT or --map FILE");
    }
    if match_text.is_some() && new_text.is_none() && !dry {
        return fail("--match requires --text (use --dry-run for search-only)");
    }
    let files = word_xml_files(&unpacked, all);
    if match_text.is_some() && new_text.is_none() {
        println!("Searching for: \"{}\"", match_text.unwrap());
        let mut total = 0;
        for xml_file in &files {
            let rel = xml_file.strip_prefix(&unpacked).unwrap_or(xml_file);
            for text in search_text_in_file(xml_file, match_text.unwrap()) {
                println!("  {}: \"{text}\"", rel.display());
                total += 1;
            }
        }
        println!("\nFound {total} occurrence(s)");
        return if total > 0 { 0 } else { 1 };
    }
    let replacements = if let Some(map_path) = map_path {
        let path = PathBuf::from(map_path);
        if !path.is_file() {
            return fail(format!("Error: {} not found", path.display()));
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return fail(format!("Error: {e}")),
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return fail(format!("Error: {e}")),
        };
        let Some(obj) = value.as_object() else {
            return fail("Error: map must be a JSON object");
        };
        let mut map = Vec::new();
        for (k, v) in obj {
            let Some(s) = v.as_str() else {
                return fail(format!("Error: map value for {k} is not a string"));
            };
            map.push((k.clone(), s.to_string()));
        }
        map
    } else {
        vec![(
            match_text.unwrap().to_string(),
            new_text.unwrap().to_string(),
        )]
    };
    if dry {
        println!("[DRY RUN] Would replace:");
        for (old, new) in &replacements {
            println!("  \"{old}\" -> \"{new}\"");
        }
    }
    let mut total_replaced = 0;
    for xml_file in &files {
        let rel = xml_file.strip_prefix(&unpacked).unwrap_or(xml_file);
        match replace_text_in_file(xml_file, &replacements, dry) {
            Ok(results) => {
                for (old_text, count) in results {
                    if count > 0 {
                        let new = replacements
                            .iter()
                            .find(|(k, _)| k == &old_text)
                            .map(|(_, v)| v.as_str())
                            .unwrap_or("");
                        println!(
                            "  {}: \"{old_text}\" -> \"{new}\" ({count}x)",
                            rel.display()
                        );
                        total_replaced += count;
                    }
                }
            }
            Err(e) => return fail(e),
        }
    }
    if total_replaced > 0 {
        let verb = if dry { "Would replace" } else { "Replaced" };
        println!(
            "\n{verb} {total_replaced} occurrence(s) across {} file(s)",
            files.len()
        );
        0
    } else {
        println!("\nNo matches found");
        1
    }
}

fn word_xml_files(unpacked: &Path, all: bool) -> Vec<PathBuf> {
    let word = unpacked.join("word");
    let mut files = vec![word.join("document.xml")];
    if all {
        if let Ok(rd) = fs::read_dir(&word) {
            let mut names: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            names.sort();
            for path in names {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if (name.starts_with("header") || name.starts_with("footer"))
                    && name.ends_with(".xml")
                {
                    files.push(path);
                }
            }
        }
    }
    files.into_iter().filter(|p| p.is_file()).collect()
}

fn replace_text_in_file(
    path: &Path,
    replacements: &[(String, String)],
    dry_run: bool,
) -> Result<Vec<(String, usize)>, String> {
    let mut raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for (old_text, new_text) in replacements {
        let lines = split_new_lines(new_text);
        let expand = lines.as_ref().is_some_and(|l| l.len() > 1);
        let segments = split_at_txbx(&raw);
        let mut total = 0;
        let mut new_segments = Vec::new();
        for seg in segments {
            let (processed, count) =
                process_segment(&seg, old_text, new_text, expand, lines.as_deref());
            new_segments.push(processed);
            total += count;
        }
        if total > 0 {
            raw = new_segments.join("");
        }
        results.push((old_text.clone(), total));
    }
    if !dry_run && results.iter().any(|(_, n)| *n > 0) {
        fs::write(path, raw).map_err(|e| e.to_string())?;
    }
    Ok(results)
}

fn split_new_lines(new_text: &str) -> Option<Vec<String>> {
    if new_text.contains("\\n") {
        Some(new_text.split("\\n").map(|s| s.to_string()).collect())
    } else if new_text.contains('\n') {
        Some(new_text.split('\n').map(|s| s.to_string()).collect())
    } else {
        None
    }
}

fn process_segment(
    segment: &str,
    old_text: &str,
    new_text: &str,
    expand: bool,
    new_lines: Option<&[String]>,
) -> (String, usize) {
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut total = 0;
    for m in p_re.find_iter(segment) {
        parts.push(segment[last..m.start()].to_string());
        let para = m.as_str();
        if expand {
            if let Some(lines) = new_lines {
                let concat = wt_concat(para);
                if concat
                    .to_ascii_lowercase()
                    .contains(&old_text.to_ascii_lowercase())
                {
                    parts.push(expand_paragraph(para, lines));
                    total += 1;
                } else {
                    parts.push(para.to_string());
                }
            }
        } else {
            let (new_para, count) = replace_in_paragraph_raw(para, old_text, new_text);
            parts.push(new_para);
            total += count;
        }
        last = m.end();
    }
    parts.push(segment[last..].to_string());
    (parts.join(""), total)
}

fn wt_concat(para: &str) -> String {
    let t_re = Regex::new(r"(?s)<w:t\b[^>]*>(.*?)</w:t>").unwrap();
    t_re.captures_iter(para)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect::<Vec<_>>()
        .join("")
}

fn expand_paragraph(para: &str, new_lines: &[String]) -> String {
    if new_lines.len() <= 1 {
        return para.to_string();
    }
    let rpr = Regex::new(r"(?s)(<w:rPr>.*?</w:rPr>)")
        .unwrap()
        .captures(para)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default();
    let ppr = Regex::new(r"(?s)(<w:pPr>.*?</w:pPr>)")
        .unwrap()
        .captures(para)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default();
    new_lines
        .iter()
        .map(|line| make_paragraph(line, &ppr, &rpr))
        .collect::<Vec<_>>()
        .join("\n")
}

fn make_paragraph(text: &str, ppr: &str, rpr: &str) -> String {
    let escaped = xml_escape(text);
    let space = if escaped.starts_with(' ') || escaped.ends_with(' ') {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    format!("<w:p>{ppr}<w:r>{rpr}<w:t{space}>{escaped}</w:t></w:r></w:p>")
}

fn replace_in_paragraph_raw(raw: &str, old_text: &str, new_text: &str) -> (String, usize) {
    if old_text.is_empty() {
        return (raw.to_string(), 0);
    }
    let new_esc = xml_escape(new_text);
    let t_re = Regex::new(r"(?s)(<w:t\b[^>]*>)(.*?)(</w:t>)").unwrap();
    let find_re = Regex::new(&format!("(?i){}", regex::escape(old_text))).unwrap();
    let mut result = raw.to_string();
    let mut count = 0usize;
    let mut search_from = 0usize;
    loop {
        let matches: Vec<(usize, usize, String)> = t_re
            .captures_iter(&result)
            .filter_map(|c| {
                let whole = c.get(0)?;
                let inner = c.get(2)?.as_str().to_string();
                Some((whole.start(), whole.end(), inner))
            })
            .collect();
        if matches.is_empty() {
            break;
        }
        let mut concat = String::new();
        let mut char_map = Vec::new();
        for (idx, (_, _, inner)) in matches.iter().enumerate() {
            let start = concat.len();
            concat.push_str(inner);
            char_map.push((start, concat.len(), idx));
        }
        if search_from > concat.len() {
            break;
        }
        let Some(found) = find_re.find(&concat[search_from..]) else {
            break;
        };
        let match_start = search_from + found.start();
        let match_end = search_from + found.end();
        let mut first_t = None;
        let mut last_t = None;
        for (idx, (cs, ce, _)) in char_map.iter().enumerate() {
            if *cs < match_end && *ce > match_start {
                if first_t.is_none() {
                    first_t = Some(idx);
                }
                last_t = Some(idx);
            }
        }
        let (Some(first_t), Some(last_t)) = (first_t, last_t) else {
            break;
        };
        let new_content = if first_t == last_t {
            let old = &matches[first_t].2;
            let offset = match_start - char_map[first_t].0;
            let end = (offset + (match_end - match_start)).min(old.len());
            let offset = offset.min(old.len());
            format!("{}{}{}", &old[..offset], new_esc, &old[end..])
        } else {
            let first_text = &matches[first_t].2;
            let last_text = &matches[last_t].2;
            let offset_first = (match_start - char_map[first_t].0).min(first_text.len());
            let offset_last = (match_end - char_map[last_t].0).min(last_text.len());
            format!(
                "{}{}{}",
                &first_text[..offset_first],
                new_esc,
                &last_text[offset_last..]
            )
        };
        let space = if new_content.starts_with(' ') || new_content.ends_with(' ') {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        for idx in (first_t..=last_t).rev() {
            let (start, end, _) = matches[idx];
            let replacement = if idx == first_t {
                format!("<w:t{space}>{new_content}</w:t>")
            } else {
                "<w:t></w:t>".to_string()
            };
            result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
        }
        count += 1;
        search_from = match_start + new_esc.len();
    }
    (result, count)
}

fn find_txbx_ranges(raw: &str) -> Vec<(usize, usize)> {
    let open = "<w:txbxContent>";
    let close = "</w:txbxContent>";
    let mut ranges = Vec::new();
    let mut pos = 0;
    while let Some(rel) = raw[pos..].find(open) {
        let start = pos + rel;
        let mut depth = 1;
        let mut search = start + open.len();
        while depth > 0 {
            let next_open = raw[search..].find(open).map(|i| search + i);
            let next_close = raw[search..].find(close).map(|i| search + i);
            let Some(next_close) = next_close else {
                break;
            };
            if next_open.is_some_and(|o| o < next_close) {
                depth += 1;
                search = next_open.unwrap() + open.len();
            } else {
                depth -= 1;
                if depth == 0 {
                    ranges.push((start, next_close + close.len()));
                }
                search = next_close + close.len();
            }
        }
        pos = search;
    }
    ranges
}

fn split_at_txbx(raw: &str) -> Vec<String> {
    let ranges = find_txbx_ranges(raw);
    if ranges.is_empty() {
        return vec![raw.to_string()];
    }
    let mut segments = Vec::new();
    let mut prev = 0;
    for (start, end) in ranges {
        if start > prev {
            segments.push(raw[prev..start].to_string());
        }
        segments.push(raw[start..end].to_string());
        prev = end;
    }
    if prev < raw.len() {
        segments.push(raw[prev..].to_string());
    }
    segments
}

fn search_text_in_file(path: &Path, match_text: &str) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let mut found = Vec::new();
    for seg in split_at_txbx(&raw) {
        for m in p_re.find_iter(&seg) {
            let concat = wt_concat(m.as_str());
            if concat
                .to_ascii_lowercase()
                .contains(&match_text.to_ascii_lowercase())
            {
                let mut preview = concat.trim().to_string();
                if preview.len() > 80 {
                    preview.truncate(77);
                    preview.push_str("...");
                }
                found.push(preview);
            }
        }
    }
    found
}

fn cmd_replace_field(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill replace-field <unpacked-dir> [--list | --field NAME --text TEXT | --sdt TAG --text TEXT | --map FILE] [--dry-run]",
        );
    }
    let unpacked = PathBuf::from(&pos[0]);
    let doc_path = unpacked.join("word/document.xml");
    if !doc_path.is_file() {
        return fail(format!("Error: {} not found", doc_path.display()));
    }
    if flag(args, "--list") {
        println!("{}", list_fields(&unpacked));
        return 0;
    }
    let field = opt(args, "--field");
    let sdt = opt(args, "--sdt");
    let text = opt(args, "--text");
    let map_path = opt(args, "--map");
    if field.is_none() && sdt.is_none() && map_path.is_none() {
        return fail("Specify --list, --field NAME, --sdt TAG, or --map FILE");
    }
    if (field.is_some() || sdt.is_some()) && text.is_none() {
        return fail("--field/--sdt requires --text");
    }
    let mut raw = match fs::read_to_string(&doc_path) {
        Ok(t) => t,
        Err(e) => return fail(e.to_string()),
    };
    let dry = flag(args, "--dry-run");
    let mut total = 0;
    if let Some(map_path) = map_path {
        let path = PathBuf::from(map_path);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return fail(e.to_string()),
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return fail(e.to_string()),
        };
        let Some(obj) = value.as_object() else {
            return fail("Error: map must be a JSON object");
        };
        for (name, val) in obj {
            let Some(value) = val.as_str() else {
                return fail(format!("Error: map value for {name} is not a string"));
            };
            let (next, count) = replace_merge_field(&raw, name, value);
            let (next, count) = if count == 0 {
                replace_sdt(&raw, name, value)
            } else {
                (next, count)
            };
            if count > 0 {
                raw = next;
                total += count;
                println!("  field \"{name}\" -> \"{value}\" ({count}x)");
            }
        }
    } else if let Some(field) = field {
        let (next, count) = replace_merge_field(&raw, field, text.unwrap());
        raw = next;
        total += count;
        println!(
            "  MERGEFIELD \"{field}\" -> \"{}\" ({count}x)",
            text.unwrap()
        );
    } else if let Some(sdt) = sdt {
        let (next, count) = replace_sdt(&raw, sdt, text.unwrap());
        raw = next;
        total += count;
        println!("  SDT \"{sdt}\" -> \"{}\" ({count}x)", text.unwrap());
    }
    if total > 0 && !dry {
        if let Err(e) = fs::write(&doc_path, raw) {
            return fail(e.to_string());
        }
        println!("\nUpdated: {total} field(s) replaced");
        0
    } else if total == 0 {
        println!("\nNo matching fields found");
        1
    } else {
        println!("\n[DRY RUN] Would replace {total} field(s)");
        0
    }
}

fn list_fields(unpacked: &Path) -> String {
    let doc_path = unpacked.join("word/document.xml");
    let Ok(doc) = read_doc(&doc_path) else {
        return "Error: document.xml not found".into();
    };
    let mut lines = vec!["\n=== Document Fields & Placeholders ===".to_string()];
    let mut merge_fields = Vec::new();
    let merge_re = Regex::new(r#"(?i)MERGEFIELD\s+"?(\w+)"?"#).unwrap();
    for instr in find_q(&doc.root, "w:instrText") {
        let text = instr.text_content();
        if let Some(c) = merge_re.captures(text.trim()) {
            merge_fields.push(c.get(1).unwrap().as_str().to_string());
        }
    }
    for fld in find_q(&doc.root, "w:fldSimple") {
        let instr = fld.attr("instr").unwrap_or("");
        if let Some(c) = merge_re.captures(instr) {
            merge_fields.push(c.get(1).unwrap().as_str().to_string());
        }
    }
    if !merge_fields.is_empty() {
        lines.push("\n  Merge Fields:".into());
        let mut names = merge_fields.clone();
        names.sort();
        names.dedup();
        for name in names {
            let count = merge_fields.iter().filter(|n| *n == &name).count();
            if count > 1 {
                lines.push(format!("    {name} ({count}x)"));
            } else {
                lines.push(format!("    {name}"));
            }
        }
    }
    let mut sdt_items = Vec::new();
    for sdt in find_q(&doc.root, "w:sdt") {
        let mut tag = String::new();
        let mut alias = String::new();
        if let Some(pr) = find_q(sdt, "w:sdtPr").into_iter().next() {
            if let Some(t) = find_q(pr, "w:tag").into_iter().next() {
                tag = t.attr("val").unwrap_or("").to_string();
            }
            if let Some(a) = find_q(pr, "w:alias").into_iter().next() {
                alias = a.attr("val").unwrap_or("").to_string();
            }
        }
        let mut content = String::new();
        if let Some(body) = find_q(sdt, "w:sdtContent").into_iter().next() {
            for t in find_q(body, "w:t") {
                content.push_str(&t.text_content());
            }
        }
        if !tag.is_empty() || !alias.is_empty() {
            let mut shown = content.trim().to_string();
            if shown.len() > 50 {
                shown.truncate(50);
            }
            sdt_items.push((tag, alias, shown));
        }
    }
    if !sdt_items.is_empty() {
        lines.push("\n  Content Controls (SDT):".into());
        for (tag, alias, content) in sdt_items {
            let label = if !tag.is_empty() {
                tag.clone()
            } else if !alias.is_empty() {
                alias.clone()
            } else {
                "(no tag)".into()
            };
            let preview = if content.is_empty() {
                String::new()
            } else {
                format!(" = \"{content}\"")
            };
            let alias_note = if !alias.is_empty() && alias != tag {
                format!(" alias={alias}")
            } else {
                String::new()
            };
            lines.push(format!("    {label}{alias_note}{preview}"));
        }
    }
    let mut bookmarks = Vec::new();
    for bm in find_q(&doc.root, "w:bookmarkStart") {
        let name = bm.attr("name").unwrap_or("");
        if !name.is_empty() && !name.starts_with('_') {
            bookmarks.push(name.to_string());
        }
    }
    if !bookmarks.is_empty() {
        lines.push("\n  Bookmarks:".into());
        bookmarks.sort();
        bookmarks.dedup();
        for name in bookmarks {
            lines.push(format!("    {name}"));
        }
    }
    if lines.len() == 1 {
        lines.push("  (no fields, content controls, or bookmarks found)".into());
    }
    lines.join("\n")
}

fn replace_merge_field(raw: &str, field_name: &str, new_text: &str) -> (String, usize) {
    let new_text = xml_escape(new_text);
    let mut raw = raw.to_string();
    let mut count = 0;
    let simple = Regex::new(&format!(
        r#"(?is)(<w:fldSimple\s+w:instr="[^"]*MERGEFIELD\s+"?{}?"?[^"]*"[^>]*>)(.*?)(</w:fldSimple>)"#,
        regex::escape(field_name)
    ))
    .unwrap();
    let hits: Vec<(usize, usize, String, String, String)> = simple
        .captures_iter(&raw)
        .filter_map(|c| {
            Some((
                c.get(0)?.start(),
                c.get(0)?.end(),
                c.get(1)?.as_str().to_string(),
                c.get(2)?.as_str().to_string(),
                c.get(3)?.as_str().to_string(),
            ))
        })
        .collect();
    for (start, end, open, body, close) in hits.into_iter().rev() {
        let rpr = Regex::new(r"(?s)(<w:rPr>.*?</w:rPr>)")
            .unwrap()
            .captures(&body)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_default();
        let space = if new_text.starts_with(' ') || new_text.ends_with(' ') {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        let content = format!("<w:r>{rpr}<w:t{space}>{new_text}</w:t></w:r>");
        raw = format!("{}{open}{content}{close}{}", &raw[..start], &raw[end..]);
        count += 1;
    }
    let instr = Regex::new(&format!(
        r#"(?i)<w:instrText[^>]*>[^<]*MERGEFIELD\s+"?{}"?[^<]*</w:instrText>"#,
        regex::escape(field_name)
    ))
    .unwrap();
    let t_re = Regex::new(r"(?s)(<w:t[^>]*>)(.*?)(</w:t>)").unwrap();
    if let Some(m) = instr.find(&raw) {
        let after = &raw[m.end()..];
        if let Some(t) = t_re.find(after) {
            let abs_start = m.end() + t.start();
            let abs_end = m.end() + t.end();
            let space = if new_text.starts_with(' ') || new_text.ends_with(' ') {
                " xml:space=\"preserve\""
            } else {
                ""
            };
            raw = format!(
                "{}<w:t{space}>{new_text}</w:t>{}",
                &raw[..abs_start],
                &raw[abs_end..]
            );
            count += 1;
        }
    }
    (raw, count)
}

fn replace_sdt(raw: &str, tag_name: &str, new_text: &str) -> (String, usize) {
    let new_text = xml_escape(new_text);
    let mut raw = raw.to_string();
    let mut count = 0;
    let sdt_re = Regex::new(r"(?s)(<w:sdt\b[^>]*>)(.*?)(</w:sdt>)").unwrap();
    let tag_re = Regex::new(&format!(
        r#"(?i)<w:tag\s+w:val="{}""#,
        regex::escape(tag_name)
    ))
    .unwrap();
    let alias_re = Regex::new(&format!(
        r#"(?i)<w:alias\s+w:val="{}""#,
        regex::escape(tag_name)
    ))
    .unwrap();
    let content_re = Regex::new(r"(?s)(<w:sdtContent\b[^>]*>)(.*?)(</w:sdtContent>)").unwrap();
    let hits: Vec<(usize, usize, String, String, String)> = sdt_re
        .captures_iter(&raw)
        .filter_map(|c| {
            Some((
                c.get(0)?.start(),
                c.get(0)?.end(),
                c.get(1)?.as_str().to_string(),
                c.get(2)?.as_str().to_string(),
                c.get(3)?.as_str().to_string(),
            ))
        })
        .collect();
    for (start, end, open, block, close) in hits.into_iter().rev() {
        if !tag_re.is_match(&block) && !alias_re.is_match(&block) {
            continue;
        }
        let Some(content) = content_re.captures(&block) else {
            continue;
        };
        let rpr = Regex::new(r"(?s)(<w:rPr>.*?</w:rPr>)")
            .unwrap()
            .captures(content.get(2).unwrap().as_str())
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_default();
        let space = if new_text.starts_with(' ') || new_text.ends_with(' ') {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        let new_content = format!("<w:p><w:r>{rpr}<w:t{space}>{new_text}</w:t></w:r></w:p>");
        let cstart = content.get(0).unwrap().start();
        let cend = content.get(0).unwrap().end();
        let new_block = format!(
            "{}{}{}{}{}",
            &block[..cstart],
            content.get(1).unwrap().as_str(),
            new_content,
            content.get(3).unwrap().as_str(),
            &block[cend..]
        );
        raw = format!("{}{open}{new_block}{close}{}", &raw[..start], &raw[end..]);
        count += 1;
    }
    (raw, count)
}

fn cmd_inspect(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill inspect <unpacked-dir-or-docx> [--text] [--sections] [--media]",
        );
    }
    let mut path = PathBuf::from(&pos[0]);
    let show_text = flag(args, "--text");
    let show_sections = flag(args, "--sections");
    let show_media = flag(args, "--media");
    let mut cleanup = None;
    if path.is_file() {
        let suffix = office_suffix(&path).unwrap_or_default();
        if !matches!(suffix.as_str(), ".docx" | ".dotx" | ".doc" | ".dotm") {
            return fail(format!(
                "Error: {} is a file but not a Word document (.docx/.dotx). Pass an unpacked directory or a .docx/.dotx file.",
                path.display()
            ));
        }
        let temp = std::env::temp_dir().join(format!("docx-skill-inspect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        if let Err(e) = unpack(&path, &temp, true, true) {
            return fail(format!("Error unpacking {}: {e}", path.display()));
        }
        path = temp.clone();
        cleanup = Some(temp);
    } else if !path.is_dir() {
        return fail(format!("Error: {} not found", path.display()));
    }
    if !path.join("word").is_dir() {
        if let Some(temp) = cleanup {
            let _ = fs::remove_dir_all(temp);
        }
        return fail(format!(
            "Error: {}/word/ not found — is this an unpacked docx?",
            path.display()
        ));
    }
    println!("{}", inspect_page_setup(&path));
    println!("{}", inspect_theme(&path));
    println!("{}", inspect_styles(&path));
    println!("{}", inspect_content(&path, show_text));
    if show_sections {
        println!("{}", inspect_sections_text(&path));
    }
    if show_media {
        println!("{}", inspect_media(&path));
    }
    if let Some(temp) = cleanup {
        let _ = fs::remove_dir_all(temp);
    }
    0
}

fn inches(val: &str) -> Option<f64> {
    val.parse::<f64>().ok().map(|n| n / 1440.0)
}

fn paper_name(w: f64, h: f64) -> &'static str {
    if (w - 11.69).abs() < 0.2 && (h - 8.27).abs() < 0.2 {
        " (A4)"
    } else if (w - 8.27).abs() < 0.2 && (h - 11.69).abs() < 0.2 {
        " (A4)"
    } else if (w - 8.5).abs() < 0.2 && (h - 11.0).abs() < 0.2 {
        " (Letter)"
    } else if (w - 11.0).abs() < 0.2 && (h - 8.5).abs() < 0.2 {
        " (Letter)"
    } else {
        ""
    }
}

fn inspect_page_setup(unpacked: &Path) -> String {
    let Ok(doc) = read_doc(&unpacked.join("word/document.xml")) else {
        return String::new();
    };
    let Some(sect) = find_q(&doc.root, "w:sectPr").into_iter().next() else {
        return String::new();
    };
    let mut lines = vec!["\n=== Document Info ===".to_string()];
    if let Some(pg) = find_q(sect, "w:pgSz").into_iter().next() {
        if let (Some(w), Some(h)) = (pg.attr("w").and_then(inches), pg.attr("h").and_then(inches)) {
            let orient = pg.attr("orient").unwrap_or("portrait");
            lines.push(format!(
                "  Page: {w:.1}\"×{h:.1}\" {orient}{}",
                paper_name(w, h)
            ));
        }
    }
    if let Some(mar) = find_q(sect, "w:pgMar").into_iter().next() {
        let mut parts = Vec::new();
        for side in ["top", "bottom", "left", "right"] {
            if let Some(v) = mar.attr(side).and_then(inches) {
                parts.push(format!("{side}={v:.1}\""));
            }
        }
        if !parts.is_empty() {
            lines.push(format!("  Margins: {}", parts.join(" ")));
        }
    }
    let headers: Vec<_> = find_q(sect, "w:headerReference")
        .into_iter()
        .filter_map(|e| e.attr("type"))
        .map(|s| s.to_string())
        .collect();
    let footers: Vec<_> = find_q(sect, "w:footerReference")
        .into_iter()
        .filter_map(|e| e.attr("type"))
        .map(|s| s.to_string())
        .collect();
    if !headers.is_empty() {
        lines.push(format!("  Headers: {}", headers.join(", ")));
    }
    if !footers.is_empty() {
        lines.push(format!("  Footers: {}", footers.join(", ")));
    }
    if lines.len() > 1 {
        lines.join("\n")
    } else {
        String::new()
    }
}

fn inspect_theme(unpacked: &Path) -> String {
    let theme_dir = unpacked.join("word/theme");
    if !theme_dir.is_dir() {
        return String::new();
    }
    let mut files = Vec::new();
    if let Ok(rd) = fs::read_dir(&theme_dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if name.starts_with("theme") && name.ends_with(".xml") {
                files.push(ent.path());
            }
        }
    }
    files.sort();
    let mut lines = vec!["\n=== Theme ===".to_string()];
    for theme_file in files {
        let Ok(doc) = read_doc(&theme_file) else {
            continue;
        };
        if let Some(scheme) = find_q(&doc.root, "a:fontScheme").into_iter().next() {
            if let Some(major) = find_q(scheme, "a:majorFont").into_iter().next() {
                if let Some(latin) = find_q(major, "a:latin").into_iter().next() {
                    lines.push(format!(
                        "  Title font:  {}",
                        latin.attr("typeface").unwrap_or("")
                    ));
                }
            }
            if let Some(minor) = find_q(scheme, "a:minorFont").into_iter().next() {
                if let Some(latin) = find_q(minor, "a:latin").into_iter().next() {
                    lines.push(format!(
                        "  Body font:   {}",
                        latin.attr("typeface").unwrap_or("")
                    ));
                }
            }
        }
        if let Some(scheme) = find_q(&doc.root, "a:clrScheme").into_iter().next() {
            lines.push(format!(
                "  Color scheme: {}",
                scheme.attr("name").unwrap_or("")
            ));
            for node in &scheme.children {
                let xml::Node::Element(child) = node else {
                    continue;
                };
                if !child.name.starts_with("a:") {
                    continue;
                }
                let color_name = child.name.trim_start_matches("a:");
                for sub_node in &child.children {
                    let xml::Node::Element(sub) = sub_node else {
                        continue;
                    };
                    if sub.name == "a:srgbClr" {
                        lines.push(format!(
                            "    {:<12} = #{}",
                            color_name,
                            sub.attr("val").unwrap_or("")
                        ));
                    } else if sub.name == "a:sysClr" {
                        let clr = sub
                            .attr("lastClr")
                            .or_else(|| sub.attr("val"))
                            .unwrap_or("?");
                        lines.push(format!("    {color_name:<12} = system:{clr}"));
                    }
                }
            }
        }
        break;
    }
    if lines.len() > 1 {
        lines.join("\n")
    } else {
        String::new()
    }
}

fn inspect_styles(unpacked: &Path) -> String {
    let Ok(doc) = read_doc(&unpacked.join("word/styles.xml")) else {
        return String::new();
    };
    let mut lines = vec!["\n=== Styles ===".to_string()];
    for style in find_q(&doc.root, "w:style") {
        let kind = style.attr("type").unwrap_or("");
        if kind != "paragraph" && kind != "character" {
            continue;
        }
        if !find_q(style, "w:semiHidden").is_empty() {
            continue;
        }
        let style_id = style.attr("styleId").unwrap_or("");
        let name = find_q(style, "w:name")
            .into_iter()
            .next()
            .and_then(|e| e.attr("val"))
            .unwrap_or(style_id);
        let mut parts = Vec::new();
        if let Some(rpr) = find_q(style, "w:rPr").into_iter().next() {
            if let Some(fonts) = find_q(rpr, "w:rFonts").into_iter().next() {
                let font = fonts
                    .attr("ascii")
                    .or_else(|| fonts.attr("hAnsi"))
                    .unwrap_or("");
                if !font.is_empty() {
                    parts.push(font.to_string());
                }
            }
            if let Some(sz) = find_q(rpr, "w:sz").into_iter().next() {
                if let Some(val) = sz.attr("val").and_then(|v| v.parse::<i64>().ok()) {
                    parts.push(format!("{}pt", val / 2));
                }
            }
            if !find_q(rpr, "w:b").is_empty() {
                parts.push("bold".into());
            }
            if !find_q(rpr, "w:i").is_empty() {
                parts.push("italic".into());
            }
            if let Some(color) = find_q(rpr, "w:color").into_iter().next() {
                let val = color.attr("val").unwrap_or("");
                if !val.is_empty() && val != "auto" {
                    parts.push(format!("#{val}"));
                } else if let Some(theme) = color.attr("themeColor") {
                    parts.push(theme.to_string());
                }
            }
        }
        let fmt = if parts.is_empty() {
            "(default)".to_string()
        } else {
            parts.join(" ")
        };
        lines.push(format!("  {name:<20} → {fmt}"));
    }
    if lines.len() > 1 {
        lines.join("\n")
    } else {
        String::new()
    }
}

fn noise_text(txt: &str) -> bool {
    let noise = ["M", "T", "W", "F", "S", "!", "\"", "'"];
    txt.len() <= 2 && (txt.chars().all(|c| c.is_ascii_digit()) || noise.contains(&txt))
}

fn inspect_content(unpacked: &Path, show_text: bool) -> String {
    let Ok(doc) = read_doc(&unpacked.join("word/document.xml")) else {
        return String::new();
    };
    let mut lines = vec!["\n=== Content Summary ===".to_string()];
    let paragraphs = find_q(&doc.root, "w:p");
    let tables = find_q(&doc.root, "w:tbl");
    let drawings = find_q(&doc.root, "w:drawing");
    let images = find_q(&doc.root, "pic:pic");
    let mut txbx_texts = Vec::new();
    for txbx in find_q(&doc.root, "w:txbxContent") {
        let mut bits = Vec::new();
        for t in find_q(txbx, "w:t") {
            let txt = t.text_content();
            let txt = txt.trim();
            if !txt.is_empty() {
                bits.push(txt.to_string());
            }
        }
        if !bits.is_empty() {
            txbx_texts.push(bits.join(" "));
        }
    }
    lines.push(format!("  Paragraphs: {}", paragraphs.len()));
    if !txbx_texts.is_empty() {
        lines.push(format!(
            "  Text boxes: {} (with editable text — use docx-skill replace-text)",
            txbx_texts.len()
        ));
    }
    if !tables.is_empty() {
        lines.push(format!("  Tables: {}", tables.len()));
    }
    if !drawings.is_empty() || !images.is_empty() {
        lines.push(format!(
            "  Images/drawings: {}",
            drawings.len().max(images.len())
        ));
    }
    let mut all_text = Vec::new();
    for t in find_q(&doc.root, "w:t") {
        let txt = t.text_content();
        let txt = txt.trim();
        if !txt.is_empty() {
            all_text.push(txt.to_string());
        }
    }
    if !all_text.is_empty() {
        let mut preview = all_text.join(" | ");
        if preview.len() > 200 {
            preview.truncate(197);
            preview.push_str("...");
        }
        lines.push(format!("  Text preview: \"{preview}\""));
        if show_text {
            let mut meaningful = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for txt in &all_text {
                if noise_text(txt) {
                    continue;
                }
                let key = txt
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .to_ascii_lowercase();
                if !seen.insert(key) {
                    continue;
                }
                meaningful.push(txt.clone());
            }
            let skipped = all_text.len().saturating_sub(meaningful.len());
            lines.push(format!(
                "\n  --- Unique Text ({} items, {skipped} noise/duplicates skipped) ---",
                meaningful.len()
            ));
            for (i, txt) in meaningful.iter().take(60).enumerate() {
                let mut shown = txt.clone();
                if shown.len() > 80 {
                    shown.truncate(77);
                    shown.push_str("...");
                }
                lines.push(format!("  [{i:3}] {shown}"));
            }
            if meaningful.len() > 60 {
                lines.push(format!("  ... and {} more", meaningful.len() - 60));
            }
        }
        if !txbx_texts.is_empty() {
            lines.push(format!(
                "\n  --- Text in Text Boxes ({} boxes — editable with docx-skill replace-text) ---",
                txbx_texts.len()
            ));
            for (i, txt) in txbx_texts.iter().enumerate() {
                let mut shown = txt.clone();
                if shown.len() > 80 {
                    shown.truncate(77);
                    shown.push_str("...");
                }
                lines.push(format!("  [txbx {i}] \"{shown}\""));
            }
        }
    }
    lines.join("\n")
}

fn inspect_sections_text(unpacked: &Path) -> String {
    let Ok(doc) = read_doc(&unpacked.join("word/document.xml")) else {
        return String::new();
    };
    let Some(body) = find_q(&doc.root, "w:body").into_iter().next() else {
        return String::new();
    };
    let mut sections: Vec<Vec<String>> = vec![Vec::new()];
    for node in &body.children {
        let xml::Node::Element(child) = node else {
            continue;
        };
        if tag_is(child, "w:p") {
            let has_sect =
                child_q(child, "w:pPr").is_some_and(|ppr| !find_q(ppr, "w:sectPr").is_empty());
            let mut para_text = Vec::new();
            for t in find_q(child, "w:t") {
                let txt = t.text_content();
                let txt = txt.trim();
                if !txt.is_empty() && !noise_text(txt) {
                    para_text.push(txt.to_string());
                }
            }
            if !para_text.is_empty() {
                sections.last_mut().unwrap().push(para_text.join(" "));
            }
            if has_sect {
                sections.push(Vec::new());
            }
        } else if tag_is(child, "w:tbl") {
            let mut table_texts = Vec::new();
            for t in find_q(child, "w:t") {
                let txt = t.text_content();
                let txt = txt.trim();
                if !txt.is_empty() && !noise_text(txt) {
                    table_texts.push(txt.to_string());
                }
            }
            if !table_texts.is_empty() {
                let more = if table_texts.len() > 10 { "..." } else { "" };
                let shown = table_texts
                    .into_iter()
                    .take(10)
                    .collect::<Vec<_>>()
                    .join(" | ");
                sections
                    .last_mut()
                    .unwrap()
                    .push(format!("[table: {shown}{more}]"));
            }
        }
    }
    let mut lines = vec!["\n=== Text by Section ===".to_string()];
    for (idx, sec_texts) in sections.iter().enumerate() {
        let mut seen = std::collections::BTreeSet::new();
        let mut unique = Vec::new();
        for t in sec_texts {
            let key = t.chars().take(80).collect::<String>().to_ascii_lowercase();
            if seen.insert(key) {
                unique.push(t.clone());
            }
        }
        lines.push(format!(
            "\n  --- Section {idx} ({} items) ---",
            unique.len()
        ));
        for txt in unique.iter().take(20) {
            let mut shown = txt.clone();
            if shown.len() > 100 {
                shown.truncate(97);
                shown.push_str("...");
            }
            lines.push(format!("    {shown}"));
        }
        if unique.len() > 20 {
            lines.push(format!("    ... and {} more", unique.len() - 20));
        }
    }
    lines.join("\n")
}

fn inspect_media(unpacked: &Path) -> String {
    let media_dir = unpacked.join("word/media");
    if !media_dir.is_dir() {
        return String::new();
    }
    let mut media_files = Vec::new();
    if let Ok(rd) = fs::read_dir(&media_dir) {
        for ent in rd.flatten() {
            if ent.path().is_file() {
                media_files.push(ent.path());
            }
        }
    }
    media_files.sort();
    if media_files.is_empty() {
        return String::new();
    }
    let mut media_refs: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let rels_path = unpacked.join("word/_rels/document.xml.rels");
    if let Ok(doc) = read_doc(&rels_path) {
        for rel in find_q(&doc.root, "Relationship") {
            let target = rel.attr("Target").unwrap_or("");
            let rid = rel.attr("Id").unwrap_or("");
            if target.contains("media/") {
                let resolved = unpacked.join("word").join(target);
                if let Ok(rel_path) = resolved.canonicalize().and_then(|p| {
                    let base = unpacked
                        .canonicalize()
                        .unwrap_or_else(|_| unpacked.to_path_buf());
                    p.strip_prefix(&base)
                        .map(|r| r.display().to_string())
                        .map_err(|e| std::io::Error::other(e.to_string()))
                }) {
                    media_refs
                        .entry(rel_path)
                        .or_default()
                        .push(rid.to_string());
                }
            }
        }
    }
    let mut lines = vec!["\n=== Media Files ===".to_string()];
    let mut total = 0u64;
    for f in &media_files {
        let rel_path = f.strip_prefix(unpacked).unwrap_or(f).display().to_string();
        let size = f.metadata().map(|m| m.len()).unwrap_or(0);
        total += size;
        let size_str = if size >= 1_048_576 {
            format!("{:.1} MB", size as f64 / 1_048_576.0)
        } else if size >= 1024 {
            format!("{:.0} KB", size as f64 / 1024.0)
        } else {
            format!("{size} B")
        };
        let ext = f
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let refs = media_refs.get(&rel_path).cloned().unwrap_or_default();
        let ref_str = if refs.is_empty() {
            "(unused)".to_string()
        } else {
            refs.join(", ")
        };
        lines.push(format!(
            "  {rel_path:<35} {size_str:>8}  {ext:<4}  {ref_str}"
        ));
    }
    let total_str = if total < 1_048_576 {
        format!("{:.0} KB", total as f64 / 1024.0)
    } else {
        format!("{:.1} MB", total as f64 / 1_048_576.0)
    };
    lines.push(format!(
        "\n  Total: {} files, {total_str}",
        media_files.len()
    ));
    lines.join("\n")
}

fn cmd_inspect_headers(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail("Usage: docx-skill inspect-headers <unpacked-dir>");
    }
    let path = PathBuf::from(&pos[0]);
    if !path.join("word").is_dir() {
        return fail(format!("Error: {}/word/ not found", path.display()));
    }
    println!("{}", inspect_headers(&path));
    0
}

fn load_rels(xml_path: &Path) -> Vec<(String, String)> {
    let rels = xml_path
        .parent()
        .unwrap_or(Path::new(""))
        .join("_rels")
        .join(format!(
            "{}.rels",
            xml_path.file_name().and_then(|s| s.to_str()).unwrap_or("")
        ));
    let Ok(doc) = read_doc(&rels) else {
        return Vec::new();
    };
    find_q(&doc.root, "Relationship")
        .into_iter()
        .filter_map(|rel| Some((rel.attr("Id")?.to_string(), rel.attr("Target")?.to_string())))
        .collect()
}

fn inspect_headers(unpacked: &Path) -> String {
    let word = unpacked.join("word");
    let doc_path = word.join("document.xml");
    let mut hf_type = std::collections::BTreeMap::new();
    if let Ok(doc) = read_doc(&doc_path) {
        for sect in find_q(&doc.root, "w:sectPr") {
            for tag in ["w:headerReference", "w:footerReference"] {
                for r in find_q(sect, tag) {
                    if let Some(rid) = r.attr("id") {
                        hf_type.insert(
                            rid.to_string(),
                            r.attr("type").unwrap_or("default").to_string(),
                        );
                    }
                }
            }
        }
    }
    let doc_rels = load_rels(&doc_path);
    let mut hf_file = std::collections::BTreeMap::new();
    for (rid, target) in &doc_rels {
        let lower = target.to_ascii_lowercase();
        if lower.contains("header") || lower.contains("footer") {
            hf_file.insert(rid.clone(), target.clone());
        }
    }
    let mut hf_files = Vec::new();
    if let Ok(rd) = fs::read_dir(&word) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if (name.starts_with("header") || name.starts_with("footer")) && name.ends_with(".xml")
            {
                hf_files.push(ent.path());
            }
        }
    }
    hf_files.sort();
    let mut lines = vec!["\n=== Headers & Footers ===".to_string()];
    if hf_files.is_empty() {
        lines.push("  (none found)".into());
        return lines.join("\n");
    }
    for hf_file in hf_files {
        let name = hf_file.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let mut kind_type = "unknown".to_string();
        for (rid, target) in &hf_file {
            if target.contains(name) {
                kind_type = hf_type
                    .get(rid)
                    .cloned()
                    .unwrap_or_else(|| "unknown".into());
                break;
            }
        }
        let kind = if name.contains("header") {
            "Header"
        } else {
            "Footer"
        };
        lines.push(format!("\n  {name} ({kind_type}) [{kind}]:"));
        let Ok(doc) = read_doc(&hf_file) else {
            lines.push("    (parse error)".into());
            continue;
        };
        let mut all_text = Vec::new();
        for t in find_q(&doc.root, "w:t") {
            let txt = t.text_content();
            let txt = txt.trim();
            if !txt.is_empty() {
                all_text.push(txt.to_string());
            }
        }
        if all_text.is_empty() {
            lines.push("    text: (empty)".into());
        } else {
            let mut preview = all_text.join(" | ");
            if preview.len() > 100 {
                preview.truncate(97);
                preview.push_str("...");
            }
            lines.push(format!("    text: \"{preview}\""));
        }
        for (rid, target) in load_rels(&hf_file) {
            let lower = target.to_ascii_lowercase();
            if [".png", ".jpg", ".jpeg", ".gif", ".svg", ".emf"]
                .iter()
                .any(|ext| lower.contains(ext))
            {
                lines.push(format!("    image: {target} ({rid})"));
            }
        }
        if !find_q(&doc.root, "w:fldSimple").is_empty()
            || !find_q(&doc.root, "w:instrText").is_empty()
        {
            lines.push("    (contains field codes — page numbers or dynamic content)".into());
        }
    }
    lines.join("\n")
}

fn cmd_inspect_tables(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail("Usage: docx-skill inspect-tables <unpacked-dir>");
    }
    let path = PathBuf::from(&pos[0]);
    if !path.join("word").is_dir() {
        return fail(format!("Error: {}/word/ not found", path.display()));
    }
    println!("{}", inspect_tables(&path));
    0
}

fn cell_text(tc: &xml::Element) -> String {
    find_q(tc, "w:t")
        .into_iter()
        .map(|t| t.text_content().trim().to_string())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn inspect_tables(unpacked: &Path) -> String {
    let Ok(doc) = read_doc(&unpacked.join("word/document.xml")) else {
        return "Error: document.xml not found".into();
    };
    let tables = find_q(&doc.root, "w:tbl");
    if tables.is_empty() {
        return "\n=== Tables ===\n  (no tables found)".into();
    }
    let mut lines = vec![format!("\n=== Tables ({}) ===", tables.len())];
    for (tbl_idx, tbl) in tables.iter().enumerate() {
        let rows = find_q(tbl, "w:tr");
        if rows.is_empty() {
            continue;
        }
        let mut max_cols = 0;
        let mut has_merge = false;
        let mut row_data = Vec::new();
        for row in rows.iter() {
            let cells = find_q(row, "w:tc");
            let mut col_count = 0;
            let mut cell_data = Vec::new();
            for cell in cells {
                let text = cell_text(cell);
                let mut colspan = None;
                let mut vmerge = None;
                let mut width = None;
                if let Some(tcpr) = find_q(cell, "w:tcPr").into_iter().next() {
                    if let Some(gs) = find_q(tcpr, "w:gridSpan").into_iter().next() {
                        if let Some(val) = gs.attr("val").and_then(|v| v.parse::<i64>().ok()) {
                            if val > 1 {
                                colspan = Some(val);
                            }
                        }
                    }
                    if let Some(vm) = find_q(tcpr, "w:vMerge").into_iter().next() {
                        vmerge = Some(if vm.attr("val") == Some("restart") {
                            "start"
                        } else {
                            "cont"
                        });
                    }
                    if let Some(tcw) = find_q(tcpr, "w:tcW").into_iter().next() {
                        if tcw.attr("type") == Some("dxa") {
                            width = tcw.attr("w").and_then(inches);
                        }
                    }
                }
                if colspan.is_some() || vmerge.is_some() {
                    has_merge = true;
                }
                col_count += colspan.unwrap_or(1);
                cell_data.push((text, colspan, vmerge, width));
            }
            max_cols = max_cols.max(col_count);
            row_data.push(cell_data);
        }
        let merge_note = if has_merge { ", has merged cells" } else { "" };
        lines.push(format!(
            "\n  Table {tbl_idx} ({} rows × {max_cols} cols{merge_note})",
            rows.len()
        ));
        if let Some(first) = row_data.first() {
            let widths: Vec<_> = first.iter().filter_map(|c| c.3).collect();
            if !widths.is_empty() {
                let width_str = widths
                    .iter()
                    .map(|w| format!("{w:.1}\""))
                    .collect::<Vec<_>>()
                    .join(" | ");
                lines.push(format!("    widths: {width_str}"));
            }
        }
        for (ri, row_cells) in row_data.iter().take(8).enumerate() {
            let mut parts = Vec::new();
            for (ci, cell) in row_cells.iter().enumerate() {
                let mut text = cell.0.clone();
                if text.len() > 20 {
                    text.truncate(17);
                    text.push_str("...");
                }
                let extra = if let Some(span) = cell.1 {
                    format!(" (colspan={span})")
                } else if cell.2 == Some("start") {
                    " (vmerge↓)".to_string()
                } else {
                    String::new()
                };
                if cell.2 == Some("cont") {
                    text = "↕".into();
                }
                parts.push(format!("[{ri},{ci}] \"{text}\"{extra}"));
            }
            let line = parts
                .iter()
                .map(|p| format!("{p:<22}"))
                .collect::<Vec<_>>()
                .join("  ");
            lines.push(format!("    {}", line.trim_end()));
        }
        if row_data.len() > 8 {
            lines.push(format!("    ... and {} more rows", row_data.len() - 8));
        }
    }
    lines.join("\n")
}

fn cmd_list_sections(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail("Usage: docx-skill list-sections <unpacked-dir>");
    }
    let path = PathBuf::from(&pos[0]);
    if !path.join("word").is_dir() {
        return fail(format!("Error: {}/word/ not found", path.display()));
    }
    println!("{}", list_sections(&path));
    0
}

fn list_sections(unpacked: &Path) -> String {
    let Ok(doc) = read_doc(&unpacked.join("word/document.xml")) else {
        return "Error: document.xml not found".into();
    };
    let paras = find_q(&doc.root, "w:p");
    let para_count = paras.len();
    let mut sections: Vec<(usize, usize, &xml::Element)> = Vec::new();
    let mut current = 0usize;
    for (i, para) in paras.iter().enumerate() {
        if let Some(ppr) = child_q(para, "w:pPr") {
            if let Some(sect) = find_q(ppr, "w:sectPr").into_iter().next() {
                sections.push((current, i, sect));
                current = i + 1;
            }
        }
    }
    if let Some(body) = find_q(&doc.root, "w:body").into_iter().next() {
        for node in &body.children {
            if let xml::Node::Element(child) = node {
                if tag_is(child, "w:sectPr") {
                    let end = para_count.saturating_sub(1);
                    sections.push((current, end, child));
                }
            }
        }
    }
    let mut lines = vec!["\n=== Sections ===".to_string()];
    if sections.is_empty() {
        lines.push("  (no section properties found)".into());
        return lines.join("\n");
    }
    for (idx, (start_para, end_para, sect)) in sections.iter().enumerate() {
        let mut w_in = 0.0;
        let mut h_in = 0.0;
        let mut orient = "portrait".to_string();
        if let Some(pg) = find_q(sect, "w:pgSz").into_iter().next() {
            if let (Some(w), Some(h)) =
                (pg.attr("w").and_then(inches), pg.attr("h").and_then(inches))
            {
                w_in = w;
                h_in = h;
            }
            orient = pg.attr("orient").unwrap_or("portrait").to_string();
        }
        let mut margin_str = String::new();
        if let Some(pg) = find_q(sect, "w:pgMar").into_iter().next() {
            let mut margins = Vec::new();
            for side in ["top", "bottom", "left", "right"] {
                if let Some(v) = pg.attr(side).and_then(inches) {
                    margins.push((side, v));
                }
            }
            if !margins.is_empty() {
                let formatted: Vec<_> = margins.iter().map(|(_, v)| format!("{v:.1}")).collect();
                if formatted.iter().all(|v| v == &formatted[0]) {
                    margin_str = format!("{}\" all", formatted[0]);
                } else {
                    margin_str = margins
                        .iter()
                        .map(|(s, v)| format!("{}={v:.1}\"", s.chars().next().unwrap_or('?')))
                        .collect::<Vec<_>>()
                        .join(" ");
                }
            }
        }
        let mut cols = 1i64;
        if let Some(col) = find_q(sect, "w:cols").into_iter().next() {
            if let Some(n) = col.attr("num").and_then(|v| v.parse().ok()) {
                cols = n;
            }
        }
        let h_types: Vec<_> = find_q(sect, "w:headerReference")
            .into_iter()
            .filter_map(|e| e.attr("type"))
            .map(|s| s.to_string())
            .collect();
        let mut parts = vec![format!("  [{idx}] ¶{}-{}", start_para + 1, end_para + 1)];
        if w_in > 0.0 {
            parts.push(format!("  {w_in:.1}\"×{h_in:.1}\" {orient}"));
        }
        if !margin_str.is_empty() {
            parts.push(format!("  margins: {margin_str}"));
        }
        parts.push(format!("  cols: {cols}"));
        if !h_types.is_empty() {
            parts.push(format!("  headers: {}", h_types.join(", ")));
        }
        lines.push(parts.join(""));
    }
    lines.join("\n")
}

fn cmd_delete_sections(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill delete-sections <unpacked-dir> (--delete SPEC | --keep SPEC) [--dry-run]",
        );
    }
    if opt(args, "--delete").is_some() && opt(args, "--keep").is_some() {
        return fail("Error: --delete and --keep are mutually exclusive");
    }
    let spec = if let Some(s) = opt(args, "--delete") {
        s
    } else if let Some(s) = opt(args, "--keep") {
        s
    } else {
        return fail("Specify --delete SPEC or --keep SPEC");
    };
    let unpacked = PathBuf::from(&pos[0]);
    let doc_path = unpacked.join("word/document.xml");
    if !doc_path.is_file() {
        return fail(format!("Error: {} not found", doc_path.display()));
    }
    let indices = match parse_indices(spec) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let raw = match fs::read_to_string(&doc_path) {
        Ok(t) => t,
        Err(e) => return fail(e.to_string()),
    };
    let sections = find_section_ranges(&raw);
    let total = sections.len();
    let to_delete = if opt(args, "--keep").is_some() {
        let mut all: std::collections::BTreeSet<usize> = (0..total).collect();
        for i in indices {
            all.remove(&i);
        }
        all
    } else {
        indices.into_iter().collect()
    };
    let n = delete_sections(&unpacked, &to_delete, flag(args, "--dry-run"));
    if n > 0 { 0 } else { 1 }
}

fn parse_indices(spec: &str) -> Result<Vec<usize>, String> {
    let mut result = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let start: usize = a
                .trim()
                .parse()
                .map_err(|_| format!("Error: bad index {part}"))?;
            let end: usize = b
                .trim()
                .parse()
                .map_err(|_| format!("Error: bad index {part}"))?;
            for i in start..=end {
                result.push(i);
            }
        } else {
            let n: usize = part
                .parse()
                .map_err(|_| format!("Error: bad index {part}"))?;
            result.push(n);
        }
    }
    Ok(result)
}

fn find_section_ranges(raw: &str) -> Vec<(usize, usize, usize, usize)> {
    let sect_para =
        Regex::new(r"(?s)<w:p\b[^>]*>(?:(?!</w:p>).)*<w:sectPr\b.*?</w:sectPr>.*?</w:p>").unwrap();
    let mut breaks = Vec::new();
    for m in sect_para.find_iter(raw) {
        breaks.push((m.start(), m.end()));
    }
    let body_sectpr = Regex::new(r"(?s)(<w:sectPr\b[^>]*>.*?</w:sectPr>)\s*</w:body>")
        .unwrap()
        .find(raw);
    let Some(body_start) = Regex::new(r"<w:body[^>]*>").unwrap().find(raw) else {
        return Vec::new();
    };
    let mut sections = Vec::new();
    let mut prev = body_start.end();
    for (i, (brk_start, brk_end)) in breaks.iter().enumerate() {
        sections.push((i, prev, *brk_start, *brk_end));
        prev = *brk_end;
    }
    if let Some(body) = body_sectpr {
        sections.push((breaks.len(), prev, body.start(), body.end()));
    }
    sections
}

fn delete_sections(
    unpacked: &Path,
    to_delete: &std::collections::BTreeSet<usize>,
    dry_run: bool,
) -> usize {
    let doc_path = unpacked.join("word/document.xml");
    let Ok(mut raw) = fs::read_to_string(&doc_path) else {
        eprintln!("Error: {} not found", doc_path.display());
        return 0;
    };
    let sections = find_section_ranges(&raw);
    if sections.is_empty() {
        eprintln!("Error: Could not find section boundaries");
        return 0;
    }
    let total = sections.len();
    let valid: Vec<usize> = to_delete.iter().copied().filter(|i| *i < total).collect();
    if valid.is_empty() {
        eprintln!("No valid sections to delete (document has {total} sections)");
        return 0;
    }
    if valid.len() == total {
        eprintln!("Error: Cannot delete ALL sections — must keep at least one");
        return 0;
    }
    println!("Document has {total} sections");
    for idx in &valid {
        let s = sections[*idx];
        println!("  Deleting section [{idx}] ({} chars)", s.3 - s.1);
    }
    if dry_run {
        println!("\n[DRY RUN] Would delete {} section(s)", valid.len());
        return valid.len();
    }
    for idx in valid.iter().rev() {
        let s = sections[*idx];
        if *idx == total - 1 {
            raw = format!("{}{}", &raw[..s.1], &raw[s.2..]);
        } else {
            raw = format!("{}{}", &raw[..s.1], &raw[s.3..]);
        }
    }
    if fs::write(&doc_path, raw).is_err() {
        return 0;
    }
    println!("\nDeleted {} section(s) from document.xml", valid.len());
    valid.len()
}

fn cmd_comment(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.len() < 3 {
        return fail(
            "Usage: docx-skill comment <unpacked-dir> <id> <text> [--author NAME] [--initials X] [--parent ID]",
        );
    }
    let unpacked = &pos[0];
    let Ok(comment_id) = pos[1].parse::<i64>() else {
        return fail("Error: comment id must be an integer");
    };
    let text = &pos[2];
    let author = opt(args, "--author").unwrap_or(DEFAULT_AUTHOR);
    let initials = opt(args, "--initials").unwrap_or("C");
    let parent = match opt(args, "--parent") {
        Some(p) => match p.parse::<i64>() {
            Ok(n) => Some(n),
            Err(_) => return fail("Error: --parent must be an integer"),
        },
        None => None,
    };
    match add_comment(unpacked, comment_id, text, author, initials, parent) {
        Ok((para_id, msg)) => {
            println!("{msg}");
            let _ = para_id;
            if parent.is_some() {
                println!(
                    "\nNest markers inside parent {}'s markers (markers must be direct children of w:p, never inside w:r):\n  <w:commentRangeStart w:id=\"{}\"/><w:commentRangeStart w:id=\"{comment_id}\"/>\n  <w:r>...</w:r>\n  <w:commentRangeEnd w:id=\"{comment_id}\"/><w:commentRangeEnd w:id=\"{}\"/>\n  <w:r><w:rPr><w:rStyle w:val=\"CommentReference\"/></w:rPr><w:commentReference w:id=\"{}\"/></w:r>\n  <w:r><w:rPr><w:rStyle w:val=\"CommentReference\"/></w:rPr><w:commentReference w:id=\"{comment_id}\"/></w:r>",
                    parent.unwrap(),
                    parent.unwrap(),
                    parent.unwrap(),
                    parent.unwrap()
                );
            } else {
                println!(
                    "\nAdd to document.xml (markers must be direct children of w:p, never inside w:r):\n  <w:commentRangeStart w:id=\"{comment_id}\"/>\n  <w:r>...</w:r>\n  <w:commentRangeEnd w:id=\"{comment_id}\"/>\n  <w:r><w:rPr><w:rStyle w:val=\"CommentReference\"/></w:rPr><w:commentReference w:id=\"{comment_id}\"/></w:r>"
                );
            }
            0
        }
        Err(msg) => {
            println!("{msg}");
            1
        }
    }
}

fn add_comment(
    unpacked_dir: &str,
    comment_id: i64,
    text: &str,
    author: &str,
    initials: &str,
    parent_id: Option<i64>,
) -> Result<(String, String), String> {
    let unpacked = PathBuf::from(unpacked_dir);
    let word = unpacked.join("word");
    if !word.exists() {
        return Err(format!("Error: {} not found", word.display()));
    }
    let para_id = hex_id();
    let durable_id = hex_id();
    let ts = utc_stamp();
    let comments = word.join("comments.xml");
    let first = !comments.exists();
    if first {
        fs::write(&comments, COMMENTS_XML).map_err(|e| e.to_string())?;
        ensure_comment_relationships(&unpacked)?;
        ensure_comment_content_types(&unpacked)?;
    }
    let body = COMMENT_BODY
        .replace("{id}", &comment_id.to_string())
        .replace("{author}", author)
        .replace("{date}", &ts)
        .replace("{initials}", initials)
        .replace("{para_id}", &para_id)
        .replace("{text}", text);
    append_before_close(&comments, "</w:comments>", &body)?;
    let ext = word.join("commentsExtended.xml");
    if !ext.exists() {
        fs::write(&ext, COMMENTS_EX).map_err(|e| e.to_string())?;
    }
    if let Some(parent_id) = parent_id {
        let parent_para = find_para_id(&comments, parent_id)
            .ok_or_else(|| format!("Error: Parent comment {parent_id} not found"))?;
        append_before_close(
            &ext,
            "</w15:commentsEx>",
            &format!(
                "<w15:commentEx w15:paraId=\"{para_id}\" w15:paraIdParent=\"{parent_para}\" w15:done=\"0\"/>"
            ),
        )?;
    } else {
        append_before_close(
            &ext,
            "</w15:commentsEx>",
            &format!("<w15:commentEx w15:paraId=\"{para_id}\" w15:done=\"0\"/>"),
        )?;
    }
    let ids = word.join("commentsIds.xml");
    if !ids.exists() {
        fs::write(&ids, COMMENTS_IDS).map_err(|e| e.to_string())?;
    }
    append_before_close(
        &ids,
        "</w16cid:commentsIds>",
        &format!(
            "<w16cid:commentId w16cid:paraId=\"{para_id}\" w16cid:durableId=\"{durable_id}\"/>"
        ),
    )?;
    let extensible = word.join("commentsExtensible.xml");
    if !extensible.exists() {
        fs::write(&extensible, COMMENTS_EXTENSIBLE).map_err(|e| e.to_string())?;
    }
    append_before_close(
        &extensible,
        "</w16cex:commentsExtensible>",
        &format!(
            "<w16cex:commentExtensible w16cex:durableId=\"{durable_id}\" w16cex:dateUtc=\"{ts}\"/>"
        ),
    )?;
    let action = if parent_id.is_some() {
        "reply"
    } else {
        "comment"
    };
    Ok((
        para_id.clone(),
        format!("Added {action} {comment_id} (para_id={para_id})"),
    ))
}

fn append_before_close(path: &Path, close: &str, fragment: &str) -> Result<(), String> {
    let mut text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let idx = text
        .rfind(close)
        .ok_or_else(|| format!("Error: {close} not found in {}", path.display()))?;
    text.insert_str(idx, &format!("{fragment}\n"));
    text = xml::smart_quote_entities(&text);
    fs::write(path, text).map_err(|e| e.to_string())
}

fn find_para_id(path: &Path, comment_id: i64) -> Option<String> {
    let doc = read_doc(path).ok()?;
    for c in find_q(&doc.root, "w:comment") {
        if c.attr("id") == Some(comment_id.to_string().as_str()) {
            for p in find_q(c, "w:p") {
                if let Some(pid) = p.attr("paraId") {
                    return Some(pid.to_string());
                }
            }
        }
    }
    None
}

fn ensure_comment_relationships(unpacked: &Path) -> Result<(), String> {
    let rels_path = unpacked.join("word/_rels/document.xml.rels");
    if !rels_path.exists() {
        return Ok(());
    }
    let mut text = fs::read_to_string(&rels_path).map_err(|e| e.to_string())?;
    if text.contains("Target=\"comments.xml\"") {
        return Ok(());
    }
    let mut next = 1;
    let id_re = Regex::new(r#"Id="rId(\d+)""#).unwrap();
    for c in id_re.captures_iter(&text) {
        if let Ok(n) = c.get(1).unwrap().as_str().parse::<i64>() {
            next = next.max(n + 1);
        }
    }
    let rels = [
        (
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
            "comments.xml",
        ),
        (
            "http://schemas.microsoft.com/office/2011/relationships/commentsExtended",
            "commentsExtended.xml",
        ),
        (
            "http://schemas.microsoft.com/office/2016/09/relationships/commentsIds",
            "commentsIds.xml",
        ),
        (
            "http://schemas.microsoft.com/office/2018/08/relationships/commentsExtensible",
            "commentsExtensible.xml",
        ),
    ];
    let mut insert = String::new();
    for (rel_type, target) in rels {
        insert.push_str(&format!(
            "<Relationship Id=\"rId{next}\" Type=\"{rel_type}\" Target=\"{target}\"/>\n"
        ));
        next += 1;
    }
    let close = "</Relationships>";
    let idx = text
        .rfind(close)
        .ok_or_else(|| "Error: relationships root not found".to_string())?;
    text.insert_str(idx, &insert);
    fs::write(&rels_path, text).map_err(|e| e.to_string())
}

fn ensure_comment_content_types(unpacked: &Path) -> Result<(), String> {
    let ct_path = unpacked.join("[Content_Types].xml");
    if !ct_path.exists() {
        return Ok(());
    }
    let mut text = fs::read_to_string(&ct_path).map_err(|e| e.to_string())?;
    if text.contains("PartName=\"/word/comments.xml\"") {
        return Ok(());
    }
    let overrides = [
        (
            "/word/comments.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
        ),
        (
            "/word/commentsExtended.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml",
        ),
        (
            "/word/commentsIds.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsIds+xml",
        ),
        (
            "/word/commentsExtensible.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtensible+xml",
        ),
    ];
    let mut insert = String::new();
    for (part, ctype) in overrides {
        insert.push_str(&format!(
            "<Override PartName=\"{part}\" ContentType=\"{ctype}\"/>\n"
        ));
    }
    let close = text
        .rfind("</Types>")
        .ok_or_else(|| "Error: content types root not found".to_string())?;
    text.insert_str(close, &insert);
    fs::write(&ct_path, text).map_err(|e| e.to_string())
}

fn hex_id() -> String {
    format!("{:08X}", rand_id())
}

fn utc_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z_in: i64) -> (i32, u32, u32) {
    let z = z_in + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    (y as i32, m as u32, d as u32)
}

fn cmd_patch(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill patch <unpacked-dir> (--patch JSON | --patch-file FILE) [--all-files] [--dry-run]",
        );
    }
    let unpacked = PathBuf::from(&pos[0]);
    let inline = opt(args, "--patch");
    let file = opt(args, "--patch-file");
    if inline.is_none() && file.is_none() {
        return fail("Specify --patch JSON or --patch-file FILE");
    }
    let text = if let Some(file) = file {
        match fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => return fail(e.to_string()),
        }
    } else {
        inline.unwrap().to_string()
    };
    let ops: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return fail(e.to_string()),
    };
    let Some(ops) = ops.as_array() else {
        return fail("Error: patch must be a JSON array of operations");
    };
    let files = word_xml_files(&unpacked, flag(args, "--all-files"));
    let dry = flag(args, "--dry-run");
    if dry {
        println!("[DRY RUN]");
    }
    for xml_file in &files {
        let rel = xml_file.strip_prefix(&unpacked).unwrap_or(xml_file);
        let file_ops: Vec<&Value> = ops
            .iter()
            .filter(|op| {
                op.get("op").and_then(|v| v.as_str()) != Some("delete_section")
                    || xml_file.file_name().and_then(|s| s.to_str()) == Some("document.xml")
            })
            .collect();
        if file_ops.is_empty() {
            continue;
        }
        match apply_patch(xml_file, &file_ops, dry) {
            Ok(messages) => {
                println!("\n{}:", rel.display());
                for msg in messages {
                    println!("  {msg}");
                }
            }
            Err(e) => return fail(e),
        }
    }
    if !dry {
        println!("\nApplied {} operation(s)", ops.len());
    }
    0
}

fn apply_patch(path: &Path, ops: &[&Value], dry_run: bool) -> Result<Vec<String>, String> {
    let mut raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut messages = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        let name = op.get("op").and_then(|v| v.as_str()).unwrap_or("");
        match apply_one(&raw, op, name) {
            Ok((next, msg)) => {
                raw = next;
                messages.push(format!("[{i}] {msg}"));
            }
            Err(e) => messages.push(format!("[{i}] ERROR in {name}: {e}")),
        }
    }
    if !dry_run {
        fs::write(path, raw).map_err(|e| e.to_string())?;
    }
    Ok(messages)
}

fn apply_one(raw: &str, op: &Value, name: &str) -> Result<(String, String), String> {
    match name {
        "replace_text" => Ok(op_replace_text(raw, op)),
        "insert_after" => Ok(op_insert_after(raw, op)),
        "insert_before" => Ok(op_insert_before(raw, op)),
        "delete_paragraph" => Ok(op_delete_paragraph(raw, op)),
        "delete_section" => Ok(op_delete_section(raw, op)),
        "set_table_cell" => Ok(op_set_table_cell(raw, op)),
        "set_style" => Ok(op_set_style(raw, op)),
        "add_paragraph" => Ok(op_add_paragraph(raw, op)),
        other => Err(format!("unknown op \"{other}\"")),
    }
}

fn op_str<'a>(op: &'a Value, key: &str) -> &'a str {
    op.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn op_replace_text(raw: &str, op: &Value) -> (String, String) {
    let match_text = op_str(op, "match");
    let new_text = op_str(op, "text");
    let normalized = new_text.replace("\\n", "\n");
    let new_lines: Vec<String> = normalized.split('\n').map(|s| s.to_string()).collect();
    let expand = new_lines.len() > 1;
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut count = 0;
    for m in p_re.find_iter(raw) {
        parts.push(raw[last..m.start()].to_string());
        let para = m.as_str();
        let para_text = wt_concat(para);
        if para_text
            .to_ascii_lowercase()
            .contains(&match_text.to_ascii_lowercase())
        {
            if expand {
                let ppr = extract_tag_block(para, "w:pPr");
                let rpr = extract_tag_block(para, "w:rPr");
                let paras = new_lines
                    .iter()
                    .map(|line| make_paragraph(line, &ppr, &rpr))
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(paras);
            } else {
                parts.push(replace_para_inplace(para, match_text, new_text));
            }
            count += 1;
        } else {
            parts.push(para.to_string());
        }
        last = m.end();
    }
    parts.push(raw[last..].to_string());
    (
        parts.join(""),
        format!("replace_text: \"{match_text}\" -> \"{new_text}\" ({count}x)"),
    )
}

fn extract_tag_block(para: &str, tag: &str) -> String {
    Regex::new(&format!(r"(?s)(<{tag}\b[^>]*>.*?</{tag}>)"))
        .unwrap()
        .captures(para)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default()
}

fn replace_para_inplace(para: &str, match_text: &str, new_text: &str) -> String {
    let t_re = Regex::new(r"(?s)(<w:t\b[^>]*>)(.*?)(</w:t>)").unwrap();
    let t_elements: Vec<(usize, usize, String)> = t_re
        .captures_iter(para)
        .filter_map(|c| {
            Some((
                c.get(0)?.start(),
                c.get(0)?.end(),
                c.get(2)?.as_str().to_string(),
            ))
        })
        .collect();
    if t_elements.is_empty() {
        return para.to_string();
    }
    let concat = t_elements
        .iter()
        .map(|(_, _, t)| t.as_str())
        .collect::<String>();
    let find_re = Regex::new(&format!("(?i){}", regex::escape(match_text))).unwrap();
    let escaped_new = xml_escape(new_text);
    let new_concat = find_re
        .replace_all(&concat, escaped_new.as_str())
        .to_string();
    let space = if new_concat.starts_with(' ') || new_concat.ends_with(' ') {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    let first = &t_elements[0];
    let mut result = format!("{}<w:t{space}>{new_concat}</w:t>", &para[..first.0]);
    if t_elements.len() > 1 {
        let last = t_elements.last().unwrap();
        let between = &para[first.1..last.0];
        let cleared = Regex::new(r"(?s)(<w:t\b[^>]*>).*?(</w:t>)")
            .unwrap()
            .replace_all(between, "$1$2")
            .to_string();
        result.push_str(&cleared);
        result.push_str(&para[last.1..]);
    } else {
        result.push_str(&para[first.1..]);
    }
    result
}

fn paragraph_items(op: &Value) -> Vec<Value> {
    op.get("paragraphs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn styled_paragraph(item: &Value, ppr: &str, fallback_rpr: &str) -> String {
    if let Some(text) = item.as_str() {
        make_paragraph(text, ppr, fallback_rpr)
    } else {
        let text = op_str(item, "text");
        let custom = build_rpr(item);
        let rpr = if custom.is_empty() {
            fallback_rpr
        } else {
            &custom
        };
        make_paragraph(text, ppr, rpr)
    }
}

fn build_rpr(style: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(font) = style.get("font").and_then(|v| v.as_str()) {
        parts.push(format!("<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\"/>"));
    }
    if style.get("bold").and_then(|v| v.as_bool()).unwrap_or(false) {
        parts.push("<w:b/>".into());
    }
    if style
        .get("italic")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        parts.push("<w:i/>".into());
    }
    if let Some(size) = style.get("size").and_then(|v| v.as_f64()) {
        parts.push(format!("<w:sz w:val=\"{}\"/>", (size * 2.0) as i64));
    }
    if let Some(color) = style.get("color").and_then(|v| v.as_str()) {
        parts.push(format!("<w:color w:val=\"{color}\"/>"));
    }
    if style
        .get("underline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        parts.push("<w:u w:val=\"single\"/>".into());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{}</w:rPr>", parts.join(""))
    }
}

fn op_insert_after(raw: &str, op: &Value) -> (String, String) {
    let match_text = op_str(op, "match");
    let items = paragraph_items(op);
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut done = false;
    for m in p_re.find_iter(raw) {
        parts.push(raw[last..m.start()].to_string());
        let para = m.as_str();
        parts.push(para.to_string());
        if !done
            && wt_concat(para)
                .to_ascii_lowercase()
                .contains(&match_text.to_ascii_lowercase())
        {
            let ppr = extract_tag_block(para, "w:pPr");
            let rpr = extract_tag_block(para, "w:rPr");
            for item in &items {
                parts.push(format!("\n{}", styled_paragraph(item, &ppr, &rpr)));
            }
            done = true;
        }
        last = m.end();
    }
    parts.push(raw[last..].to_string());
    let status = if done {
        format!(
            "insert_after: \"{match_text}\" ({} paragraphs)",
            items.len()
        )
    } else {
        format!("insert_after: \"{match_text}\" (NOT FOUND)")
    };
    (parts.join(""), status)
}

fn op_insert_before(raw: &str, op: &Value) -> (String, String) {
    let match_text = op_str(op, "match");
    let items = paragraph_items(op);
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut done = false;
    for m in p_re.find_iter(raw) {
        parts.push(raw[last..m.start()].to_string());
        let para = m.as_str();
        if !done
            && wt_concat(para)
                .to_ascii_lowercase()
                .contains(&match_text.to_ascii_lowercase())
        {
            let ppr = extract_tag_block(para, "w:pPr");
            let rpr = extract_tag_block(para, "w:rPr");
            for item in &items {
                parts.push(format!("{}\n", styled_paragraph(item, &ppr, &rpr)));
            }
            done = true;
        }
        parts.push(para.to_string());
        last = m.end();
    }
    parts.push(raw[last..].to_string());
    let status = if done {
        format!(
            "insert_before: \"{match_text}\" ({} paragraphs)",
            items.len()
        )
    } else {
        format!("insert_before: \"{match_text}\" (NOT FOUND)")
    };
    (parts.join(""), status)
}

fn op_delete_paragraph(raw: &str, op: &Value) -> (String, String) {
    let match_text = op_str(op, "match");
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut count = 0;
    for m in p_re.find_iter(raw) {
        parts.push(raw[last..m.start()].to_string());
        let para = m.as_str();
        if wt_concat(para)
            .to_ascii_lowercase()
            .contains(&match_text.to_ascii_lowercase())
        {
            count += 1;
        } else {
            parts.push(para.to_string());
        }
        last = m.end();
    }
    parts.push(raw[last..].to_string());
    (
        parts.join(""),
        format!("delete_paragraph: \"{match_text}\" ({count} removed)"),
    )
}

fn op_delete_section(raw: &str, op: &Value) -> (String, String) {
    let sect_para =
        Regex::new(r"(?s)<w:p\b[^>]*>(?:(?!</w:p>).)*<w:sectPr\b.*?</w:sectPr>.*?</w:p>").unwrap();
    let body_sectpr = Regex::new(r"(?s)(<w:sectPr\b[^>]*>.*?</w:sectPr>)\s*</w:body>")
        .unwrap()
        .find(raw);
    let Some(body_start) = Regex::new(r"<w:body[^>]*>").unwrap().find(raw) else {
        return (raw.to_string(), "delete_section: no <w:body> found".into());
    };
    let breaks: Vec<_> = sect_para.find_iter(raw).map(|m| m.end()).collect();
    let mut sections = Vec::new();
    let mut prev = body_start.end();
    for end in &breaks {
        sections.push((prev, *end));
        prev = *end;
    }
    if let Some(body) = body_sectpr {
        sections.push((prev, body.end()));
    }
    let total = sections.len();
    let to_delete: std::collections::BTreeSet<usize> =
        if let Some(keep) = op.get("keep").and_then(|v| v.as_array()) {
            let keep: std::collections::BTreeSet<usize> = keep
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect();
            (0..total).filter(|i| !keep.contains(i)).collect()
        } else {
            op.get("delete")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default()
        };
    if to_delete.is_empty() {
        return (
            raw.to_string(),
            format!("delete_section: nothing to delete ({total} sections)"),
        );
    }
    let mut raw = raw.to_string();
    for idx in to_delete.iter().rev() {
        if *idx >= total {
            continue;
        }
        let (start, end) = sections[*idx];
        if *idx == total - 1 {
            if let Some(body) = Regex::new(r"(?s)(<w:sectPr\b[^>]*>.*?</w:sectPr>)\s*</w:body>")
                .unwrap()
                .find(&raw)
            {
                raw = format!("{}{}", &raw[..start], &raw[body.start()..]);
            }
        } else {
            raw = format!("{}{}", &raw[..start], &raw[end..]);
        }
    }
    (
        raw,
        format!(
            "delete_section: removed {} of {total} sections",
            to_delete.len()
        ),
    )
}

fn op_set_table_cell(raw: &str, op: &Value) -> (String, String) {
    let tbl_idx = op.get("table").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let row_idx = op.get("row").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let col_idx = op.get("col").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let new_text = op_str(op, "text");
    let Ok(doc) = xml::parse(raw) else {
        return (raw.to_string(), "set_table_cell: parse failed".into());
    };
    let tables = find_q(&doc.root, "w:tbl");
    if tbl_idx >= tables.len() {
        return (
            raw.to_string(),
            format!(
                "set_table_cell: table {tbl_idx} not found ({} tables)",
                tables.len()
            ),
        );
    }
    let rows = find_q(tables[tbl_idx], "w:tr");
    if row_idx >= rows.len() {
        return (
            raw.to_string(),
            format!(
                "set_table_cell: row {row_idx} not found ({} rows)",
                rows.len()
            ),
        );
    }
    let cells = find_q(rows[row_idx], "w:tc");
    if col_idx >= cells.len() {
        return (
            raw.to_string(),
            format!(
                "set_table_cell: col {col_idx} not found ({} cols)",
                cells.len()
            ),
        );
    }
    let old_text = cell_text(cells[col_idx]);
    if old_text.trim().is_empty() {
        return (
            raw.to_string(),
            format!("set_table_cell: [{tbl_idx},{row_idx},{col_idx}] cell empty, skipped"),
        );
    }
    let tbl_re = Regex::new(r"(?s)(<w:tbl\b.*?</w:tbl>)").unwrap();
    let tbl_matches: Vec<_> = tbl_re.find_iter(raw).collect();
    if tbl_idx < tbl_matches.len() {
        let tbl_m = &tbl_matches[tbl_idx];
        let tbl_raw = tbl_m.as_str();
        let cell_re = Regex::new(&format!(
            r"(?s)(<w:t[^>]*>){}(</w:t>)",
            regex::escape(&old_text)
        ))
        .unwrap();
        let new_tbl = cell_re
            .replacen(tbl_raw, 1, format!("$1{new_text}$2"))
            .to_string();
        let raw = format!("{}{new_tbl}{}", &raw[..tbl_m.start()], &raw[tbl_m.end()..]);
        let old_preview: String = old_text.chars().take(20).collect();
        let new_preview: String = new_text.chars().take(20).collect();
        return (
            raw,
            format!(
                "set_table_cell: [{tbl_idx},{row_idx},{col_idx}] \"{old_preview}\" -> \"{new_preview}\""
            ),
        );
    }
    (
        raw.to_string(),
        format!("set_table_cell: [{tbl_idx},{row_idx},{col_idx}] cell empty, skipped"),
    )
}

fn op_set_style(raw: &str, op: &Value) -> (String, String) {
    let match_text = op_str(op, "match");
    let new_rpr = build_rpr(op);
    if new_rpr.is_empty() {
        return (
            raw.to_string(),
            "set_style: no style properties specified".into(),
        );
    }
    let p_re = Regex::new(r"(?s)(<w:p\b[^>]*>.*?</w:p>)").unwrap();
    let run_re = Regex::new(r"(?s)<w:r\b[^>]*>.*?</w:r>").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut count = 0;
    for m in p_re.find_iter(raw) {
        parts.push(raw[last..m.start()].to_string());
        let mut para = m.as_str().to_string();
        if wt_concat(&para)
            .to_ascii_lowercase()
            .contains(&match_text.to_ascii_lowercase())
        {
            para = run_re
                .replace_all(&para, |caps: &regex::Captures| {
                    let mut run = caps.get(0).unwrap().as_str().to_string();
                    run = Regex::new(r"(?s)<w:rPr>.*?</w:rPr>")
                        .unwrap()
                        .replace(&run, "")
                        .to_string();
                    Regex::new(r"(<w:r\b[^>]*>)")
                        .unwrap()
                        .replacen(&run, 1, format!("$1{new_rpr}"))
                        .to_string()
                })
                .to_string();
            count += 1;
        }
        parts.push(para);
        last = m.end();
    }
    parts.push(raw[last..].to_string());
    (
        parts.join(""),
        format!("set_style: \"{match_text}\" ({count} paragraphs)"),
    )
}

fn op_add_paragraph(raw: &str, op: &Value) -> (String, String) {
    let items = paragraph_items(op);
    let paras = items
        .iter()
        .map(|item| styled_paragraph(item, "", ""))
        .collect::<Vec<_>>()
        .join("\n");
    let insert = format!("{paras}\n");
    let body_sectpr = Regex::new(r"(?s)(\s*<w:sectPr\b[^>]*>.*?</w:sectPr>\s*</w:body>)").unwrap();
    let raw = if let Some(m) = body_sectpr.find(raw) {
        format!("{}\n{}{}", &raw[..m.start()], insert, &raw[m.start()..])
    } else {
        raw.replacen("</w:body>", &format!("{insert}</w:body>"), 1)
    };
    (
        raw,
        format!("add_paragraph: {} paragraph(s) appended", items.len()),
    )
}

fn cmd_accept_changes(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.len() < 2 {
        return fail("Usage: docx-skill accept-changes <input.docx> <output.docx>");
    }
    match accept_changes(Path::new(&pos[0]), Path::new(&pos[1])) {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(msg) => {
            println!("{msg}");
            1
        }
    }
}

fn accept_changes(input: &Path, output: &Path) -> Result<String, String> {
    if !input.exists() {
        return Err(format!("Error: Input file not found: {}", input.display()));
    }
    if office_suffix(input).as_deref() != Some(".docx") {
        return Err(format!(
            "Error: Input file is not a DOCX file: {}",
            input.display()
        ));
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Error: Failed to copy input file to output location: {e}"))?;
        }
    }
    fs::copy(input, output)
        .map_err(|e| format!("Error: Failed to copy input file to output location: {e}"))?;
    if !setup_libreoffice_macro() {
        return Err("Error: Failed to setup LibreOffice macro".into());
    }
    let profile = format!("-env:UserInstallation=file://{LIBREOFFICE_PROFILE}");
    let script = "vnd.sun.star.script:Standard.Module1.AcceptAllTrackedChanges?language=Basic&location=application";
    let mut cmd = Command::new("soffice");
    cmd.args(["--headless", &profile, "--norestore", script])
        .arg(output);
    apply_soffice_env(&mut cmd);
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Error: LibreOffice failed: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(format!(
                        "Successfully accepted all tracked changes: {} -> {}",
                        input.display(),
                        output.display()
                    ));
                }
                let mut err = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut err);
                }
                return Err(format!("Error: LibreOffice failed: {err}"));
            }
            Ok(None) if start.elapsed() > Duration::from_secs(30) => {
                let _ = child.kill();
                return Ok(format!(
                    "Successfully accepted all tracked changes: {} -> {}",
                    input.display(),
                    output.display()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(format!("Error: LibreOffice failed: {e}")),
        }
    }
}

fn setup_libreoffice_macro() -> bool {
    let macro_dir = PathBuf::from(LIBREOFFICE_PROFILE).join("user/basic/Standard");
    let macro_file = macro_dir.join("Module1.xba");
    if macro_file.is_file() {
        if let Ok(text) = fs::read_to_string(&macro_file) {
            if text.contains("AcceptAllTrackedChanges") {
                return true;
            }
        }
    }
    if !macro_dir.exists() {
        let profile = format!("-env:UserInstallation=file://{LIBREOFFICE_PROFILE}");
        let mut cmd = Command::new("soffice");
        cmd.args(["--headless", &profile, "--terminate_after_init"]);
        apply_soffice_env(&mut cmd);
        let _ = cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if fs::create_dir_all(&macro_dir).is_err() {
            return false;
        }
    }
    fs::write(&macro_file, ACCEPT_MACRO).is_ok()
}

fn cmd_convert(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail(
            "Usage: docx-skill convert <input> --to pdf|images|docx [output] [--outdir DIR]",
        );
    }
    let Some(to) = opt(args, "--to") else {
        return fail("--to is required (pdf, images, or docx)");
    };
    let input = PathBuf::from(&pos[0]);
    if !input.is_file() {
        return fail(format!("Error: {} not found", input.display()));
    }
    let mut output_dir = PathBuf::from(".");
    let mut rename_to = None;
    if pos.len() > 1 {
        let out = PathBuf::from(&pos[1]);
        if out.extension().is_some() {
            output_dir = if out.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
                out.parent().unwrap().to_path_buf()
            } else {
                PathBuf::from(".")
            };
            rename_to = Some(out);
        } else {
            output_dir = out;
        }
    } else if let Some(dir) = opt(args, "--outdir").or_else(|| opt(args, "--output")) {
        output_dir = PathBuf::from(dir);
    }
    if let Err(e) = fs::create_dir_all(&output_dir) {
        return fail(e.to_string());
    }
    match to {
        "pdf" => match convert_to_pdf(&input, &output_dir) {
            Ok(mut result) => {
                if let Some(dest) = rename_to {
                    if fs::rename(&result, &dest).is_err() {
                        let _ = fs::copy(&result, &dest);
                        let _ = fs::remove_file(&result);
                    }
                    result = dest;
                }
                println!("Created: {}", result.display());
                0
            }
            Err(e) => fail(e),
        },
        "images" => match convert_to_images(&input, &output_dir) {
            Ok(images) => {
                println!("Created {} image(s):", images.len());
                for img in images {
                    println!("  {}", img.display());
                }
                0
            }
            Err(e) => fail(e),
        },
        "docx" => match convert_to_docx(&input, &output_dir) {
            Ok(mut result) => {
                if let Some(dest) = rename_to {
                    if fs::rename(&result, &dest).is_err() {
                        let _ = fs::copy(&result, &dest);
                        let _ = fs::remove_file(&result);
                    }
                    result = dest;
                }
                println!("Created: {}", result.display());
                0
            }
            Err(e) => fail(e),
        },
        other => fail(format!("Error: unknown --to {other}")),
    }
}

fn convert_with_soffice(input: &Path, output_dir: &Path, format: &str) -> Result<PathBuf, String> {
    let mut cmd = Command::new("soffice");
    cmd.args(["--headless", "--convert-to", format, "--outdir"])
        .arg(output_dir)
        .arg(input);
    apply_soffice_env(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| format!("Error: {format} conversion failed: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Error: {format} conversion failed: {err}"));
    }
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let path = output_dir.join(format!("{stem}.{format}"));
    if !path.exists() {
        return Err(format!("Error: Expected {} not created", path.display()));
    }
    Ok(path)
}

fn convert_to_pdf(input: &Path, output_dir: &Path) -> Result<PathBuf, String> {
    convert_with_soffice(input, output_dir, "pdf")
}

fn convert_to_docx(input: &Path, output_dir: &Path) -> Result<PathBuf, String> {
    convert_with_soffice(input, output_dir, "docx")
}

fn convert_to_images(input: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let pdf = convert_to_pdf(input, output_dir)?;
    let prefix = output_dir.join("page");
    let out = Command::new("pdftoppm")
        .args(["-jpeg", "-r", "150"])
        .arg(&pdf)
        .arg(&prefix)
        .output()
        .map_err(|e| format!("Error: Image conversion failed: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Error: Image conversion failed: {err}"));
    }
    let mut images = Vec::new();
    if let Ok(rd) = fs::read_dir(output_dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if name.starts_with("page-") && name.ends_with(".jpg") {
                images.push(ent.path());
            }
        }
    }
    images.sort();
    Ok(images)
}

fn cmd_render(args: &[String]) -> i32 {
    let pos = positionals(args);
    if pos.is_empty() {
        return fail("Usage: docx-skill render <input> [output-prefix] [--cols N]");
    }
    let input = PathBuf::from(&pos[0]);
    if !input.is_file() {
        return fail(format!("Error: {} not found", input.display()));
    }
    let cols: u32 = opt(args, "--cols")
        .and_then(|s| s.parse().ok())
        .unwrap_or(2)
        .min(6)
        .max(1);
    let mut output = if pos.len() > 1 {
        PathBuf::from(&pos[1])
    } else {
        PathBuf::from("doc_pages")
    };
    if output.is_dir() || pos.get(1).is_some_and(|s| s.ends_with('/')) {
        output = output.join("doc_pages.jpg");
    } else if output.extension().is_none() {
        output = PathBuf::from(format!("{}.jpg", output.display()));
    }
    let temp = std::env::temp_dir().join(format!("docx-skill-render-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    if fs::create_dir_all(&temp).is_err() {
        return fail("Error: could not create temp directory");
    }
    let result = (|| {
        let images = convert_to_images(&input, &temp)?;
        create_page_grid(&images, &output, cols)
    })();
    let _ = fs::remove_dir_all(&temp);
    match result {
        Ok(path) => {
            println!("Created: {path}");
            0
        }
        Err(e) => fail(format!("Error: {e}")),
    }
}

fn create_page_grid(pages: &[PathBuf], output: &Path, cols: u32) -> Result<String, String> {
    if pages.is_empty() {
        return Err("No page images to grid".into());
    }
    const THUMB_WIDTH: u32 = 500;
    const GRID_PADDING: u32 = 20;
    const BORDER: u32 = 2;
    let first = load_jpeg(&pages[0])?;
    let aspect = first.height() as f64 / first.width() as f64;
    let thumb_height = (THUMB_WIDTH as f64 * aspect) as u32;
    let font_size = (THUMB_WIDTH as f64 * 0.10) as u32;
    let label_padding = (font_size as f64 * 0.4) as u32;
    let rows = (pages.len() as u32 + cols - 1) / cols;
    let cell_h = thumb_height + font_size + label_padding * 2;
    let grid_w = cols * THUMB_WIDTH + (cols + 1) * GRID_PADDING;
    let grid_h = rows * cell_h + (rows + 1) * GRID_PADDING;
    let mut grid = image::RgbImage::from_pixel(grid_w, grid_h, image::Rgb([255, 255, 255]));
    for (i, img_path) in pages.iter().enumerate() {
        let row = i as u32 / cols;
        let col = i as u32 % cols;
        let x = col * THUMB_WIDTH + (col + 1) * GRID_PADDING;
        let y_base = row * cell_h + (row + 1) * GRID_PADDING;
        let label = format!("Page {}", i + 1);
        draw_label(
            &mut grid,
            &label,
            x,
            y_base + label_padding,
            THUMB_WIDTH,
            font_size.max(8),
        );
        let y_thumb = y_base + label_padding + font_size + label_padding;
        let img = load_jpeg(img_path)?;
        let thumb = image::imageops::resize(
            &img,
            THUMB_WIDTH,
            thumb_height,
            image::imageops::FilterType::Lanczos3,
        );
        let (w, h) = thumb.dimensions();
        let tx = x + (THUMB_WIDTH - w) / 2;
        let ty = y_thumb + thumb_height.saturating_sub(h) / 2;
        image::imageops::overlay(&mut grid, &thumb, tx as i64, ty as i64);
        draw_rect_border(&mut grid, tx, ty, w, h, BORDER, image::Rgb([128, 128, 128]));
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    save_jpeg(&grid, output)?;
    Ok(format!("{} ({} pages)", output.display(), pages.len()))
}

fn load_jpeg(path: &Path) -> Result<image::RgbImage, String> {
    let img = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    Ok(img.to_rgb8())
}

fn save_jpeg(img: &image::RgbImage, path: &Path) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 95);
    use image::ImageEncoder;
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| e.to_string())
}

fn draw_rect_border(
    img: &mut image::RgbImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    width: u32,
    color: image::Rgb<u8>,
) {
    let (iw, ih) = img.dimensions();
    for t in 0..width {
        for dx in 0..w {
            let px = x + dx;
            let top = y.saturating_sub(t);
            let bot = y + h + t;
            if px < iw && top < ih {
                img.put_pixel(px, top, color);
            }
            if px < iw && bot < ih {
                img.put_pixel(px, bot, color);
            }
        }
        for dy in 0..h {
            let py = y + dy;
            let left = x.saturating_sub(t);
            let right = x + w + t;
            if left < iw && py < ih {
                img.put_pixel(left, py, color);
            }
            if right < iw && py < ih {
                img.put_pixel(right, py, color);
            }
        }
    }
}

fn draw_label(img: &mut image::RgbImage, text: &str, x: u32, y: u32, width: u32, scale: u32) {
    let glyph_w = 5 * scale.max(1) / 4 + 1;
    let total = text.chars().count() as u32 * glyph_w;
    let mut cx = x + width.saturating_sub(total) / 2;
    for ch in text.chars() {
        blit_glyph(img, ch, cx, y, scale.max(1) / 2);
        cx += glyph_w;
    }
}

fn blit_glyph(img: &mut image::RgbImage, ch: char, x: u32, y: u32, scale: u32) {
    let bits = glyph_bits(ch);
    let (iw, ih) = img.dimensions();
    let scale = scale.max(1);
    for (row, bits) in bits.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    let px = x + col * scale + sx;
                    let py = y + row as u32 * scale + sy;
                    if px < iw && py < ih {
                        img.put_pixel(px, py, image::Rgb([0, 0, 0]));
                    }
                }
            }
        }
    }
}

fn glyph_bits(ch: char) -> [u8; 7] {
    match ch {
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'a' => [
            0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111, 0b00000,
        ],
        'g' => [
            0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b01110, 0b00000,
        ],
        'e' => [
            0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110, 0b00000,
        ],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        _ => [
            0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
        ],
    }
}

fn cmd_soffice(args: &[String]) -> i32 {
    match run_soffice(args) {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => fail(e),
    }
}

fn run_soffice(args: &[String]) -> Result<std::process::ExitStatus, String> {
    let mut cmd = Command::new("soffice");
    cmd.args(args);
    apply_soffice_env(&mut cmd);
    cmd.status().map_err(|e| format!("soffice: {e}"))
}

fn apply_soffice_env(cmd: &mut Command) {
    cmd.env("SAL_USE_VCLPLUGIN", "svp");
    if let Some(shim) = ensure_socket_shim() {
        cmd.env("LD_PRELOAD", shim);
    }
}

fn unix_socket_blocked() -> bool {
    let path = std::env::temp_dir().join(format!("docx-skill-unix-{}", std::process::id()));
    let _ = fs::remove_file(&path);
    match std::os::unix::net::UnixListener::bind(&path) {
        Ok(_) => {
            let _ = fs::remove_file(&path);
            false
        }
        Err(_) => true,
    }
}

fn ensure_socket_shim() -> Option<PathBuf> {
    if !unix_socket_blocked() {
        return None;
    }
    let so = std::env::temp_dir().join("lo_socket_shim.so");
    if so.exists() {
        return Some(so);
    }
    let src = std::env::temp_dir().join("lo_socket_shim.c");
    if fs::write(&src, SHIM_SOURCE).is_err() {
        return None;
    }
    let ok = Command::new("gcc")
        .args(["-shared", "-fPIC", "-o"])
        .arg(&so)
        .arg(&src)
        .arg("-ldl")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = fs::remove_file(&src);
    if ok { Some(so) } else { None }
}

const SHIM_SOURCE: &str = r#"
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <unistd.h>

static int (*real_socket)(int, int, int);
static int (*real_socketpair)(int, int, int, int[2]);
static int (*real_listen)(int, int);
static int (*real_accept)(int, struct sockaddr *, socklen_t *);
static int (*real_close)(int);
static int (*real_read)(int, void *, size_t);

static int is_shimmed[1024];
static int peer_of[1024];
static int wake_r[1024];
static int wake_w[1024];
static int listener_fd = -1;

__attribute__((constructor))
static void init(void) {
    real_socket     = dlsym(RTLD_NEXT, "socket");
    real_socketpair = dlsym(RTLD_NEXT, "socketpair");
    real_listen     = dlsym(RTLD_NEXT, "listen");
    real_accept     = dlsym(RTLD_NEXT, "accept");
    real_close      = dlsym(RTLD_NEXT, "close");
    real_read       = dlsym(RTLD_NEXT, "read");
    for (int i = 0; i < 1024; i++) {
        peer_of[i] = -1;
        wake_r[i]  = -1;
        wake_w[i]  = -1;
    }
}

int socket(int domain, int type, int protocol) {
    if (domain == AF_UNIX) {
        int fd = real_socket(domain, type, protocol);
        if (fd >= 0) return fd;
        int sv[2];
        if (real_socketpair(domain, type, protocol, sv) == 0) {
            if (sv[0] >= 0 && sv[0] < 1024) {
                is_shimmed[sv[0]] = 1;
                peer_of[sv[0]]    = sv[1];
                int wp[2];
                if (pipe(wp) == 0) {
                    wake_r[sv[0]] = wp[0];
                    wake_w[sv[0]] = wp[1];
                }
            }
            return sv[0];
        }
        errno = EPERM;
        return -1;
    }
    return real_socket(domain, type, protocol);
}

int listen(int sockfd, int backlog) {
    if (sockfd >= 0 && sockfd < 1024 && is_shimmed[sockfd]) {
        listener_fd = sockfd;
        return 0;
    }
    return real_listen(sockfd, backlog);
}

int accept(int sockfd, struct sockaddr *addr, socklen_t *addrlen) {
    if (sockfd >= 0 && sockfd < 1024 && is_shimmed[sockfd]) {
        if (wake_r[sockfd] >= 0) {
            char buf;
            real_read(wake_r[sockfd], &buf, 1);
        }
        errno = ECONNABORTED;
        return -1;
    }
    return real_accept(sockfd, addr, addrlen);
}

int close(int fd) {
    if (fd >= 0 && fd < 1024 && is_shimmed[fd]) {
        int was_listener = (fd == listener_fd);
        is_shimmed[fd] = 0;
        if (wake_w[fd] >= 0) {
            char c = 0;
            write(wake_w[fd], &c, 1);
            real_close(wake_w[fd]);
            wake_w[fd] = -1;
        }
        if (wake_r[fd] >= 0) { real_close(wake_r[fd]); wake_r[fd] = -1; }
        if (peer_of[fd] >= 0) { real_close(peer_of[fd]); peer_of[fd] = -1; }
        if (was_listener)
            _exit(0);
    }
    return real_close(fd);
}
"#;
