//! Handle creating the debugger dependent on the type of debugger specified
//!
//! See core/debugger.rs for more centric/shared debugger material once one is created

pub struct Debugger {}

impl Debugger {
    /// Get the debugger implementation
    ///
    /// If the debugger type is not specified it will try it's best to guess what kind of debugger to
    /// return.
    pub fn new(
        debugger_cmd: Option<&str>,
        debugger_type: Option<&str>,
        run_cmd: Vec<String>,
    ) -> Self {
        Debugger {}
    }
}
