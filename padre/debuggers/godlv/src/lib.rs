//! The Go Delve debugger module

#[macro_use]
extern crate lazy_static;

mod debugger;
mod process;

pub use self::debugger::ImplDebugger;
