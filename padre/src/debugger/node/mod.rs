//! The Node debugger module

mod analyser;
mod debugger;
mod process;
mod ws;

pub use self::debugger::ImplDebugger;
