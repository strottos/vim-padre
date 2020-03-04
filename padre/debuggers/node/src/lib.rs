//! The Node debugger module

#[macro_use]
extern crate lazy_static;

mod analyser;
mod debugger;
mod process;
mod utils;
mod ws;

pub use self::debugger::ImplDebugger;
