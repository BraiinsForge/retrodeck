//! Standalone dashboard application. No Lisp runtime or FFI is used here.
pub mod app;
pub mod cli;
pub mod credits;
pub mod menu;
#[cfg(test)]
mod parity_tests;
pub mod policy;
pub mod runtime;
pub mod settings;
pub mod ui;
pub mod wifi;

pub fn run(arguments: impl IntoIterator<Item = String>) -> i32 {
    runtime::main(arguments)
}
