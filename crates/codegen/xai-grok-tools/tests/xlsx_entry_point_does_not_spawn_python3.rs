//! The XLSX entry point lives in `src/util/xlsx_skill.rs`.
//! This file is the test crate so that module is compiled without editing
//! a tracked `mod` line. `#[cfg(test)]` in that module is on for this crate.
//! The named test is `xlsx_skill::tests::xlsx_entry_point_does_not_spawn_python3`.

#[path = "../src/util/xlsx_skill.rs"]
mod xlsx_skill;
