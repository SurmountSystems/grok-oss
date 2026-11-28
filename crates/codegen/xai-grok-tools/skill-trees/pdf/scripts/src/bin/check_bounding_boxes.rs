fn main() {
    if let Err(err) = pdf_skill::bbox::check_bounding_boxes_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
