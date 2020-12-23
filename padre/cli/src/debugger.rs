//! Handle creating the debugger dependent on the type of debugger specified
//!
//! See core/debugger.rs for more centric/shared debugger material once one is created

/// Get the debugger implementation
///
/// If the debugger type is not specified it will try it's best to guess what kind of debugger to
/// return.
pub fn create_debugger(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
) -> Box<String> {
    Box::new("test".to_string())
}
