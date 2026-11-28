//! Entry point for the memory integration suite.
//!
//! No recipe, workflow, or process command executes the former suite file.
//! The case bodies were removed for that reason. They were not reimplemented
//! here. This process does not run those cases.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("memory integration case bodies were removed because no caller executed them");
    eprintln!("this process does not run those cases");
    ExitCode::SUCCESS
}
