fn main() {
    if let Err(err) = pdf_skill::forms::check_fillable_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
