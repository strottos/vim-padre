//! Configuration
//!
//! Responsible for reading and setting config items and setting default configs
//!
//! The following config items can be set:
//!  - BackPressure: Set the backpressure of the queue to build up. 0 means it errors on any
//!    request when one is in progress. Defaults to 20.
//!  - UnknownPosition: Set to the following:
//!    0: if we should halt when we reach an unknown position
//!    1: if we should carry on stepping over until we reach a known position.
//!    2: if we should carry on stepping in until we reach a known position.
//!  - ProcessSpawnTimeout: Set the timeout value for spawniong a process. Defaults
//!    to 10 seconds.
//!  - BreakpointTimeout: Timeout for setting a breakpoint. Defaults to 2 second.
//!    Only used in LLDB.
//!  - PrintVariableTimeout: Timeout for setting a breakpoint. Defaults to 2 second.
//!    Only used in LLDB.

use std::collections::HashMap;

/// Configuration
///
/// Each socket that is opened will create a new configuration that
/// can be altered per socket.
///
/// Only config items that are meaningful and have defaults can be set and
/// retreived.
#[derive(Clone, Debug)]
pub struct Config<'a> {
    config: HashMap<&'a str, i64>,
}

impl<'a> Config<'a> {
    pub fn new() -> Self {
        let mut config = HashMap::new();
        config.insert("BackPressure", 20);
        config.insert("UnknownPosition", 0);
        config.insert("ProcessSpawnTimeout", 10);
        config.insert("BreakpointTimeout", 2);
        config.insert("PrintVariableTimeout", 2);
        Config { config }
    }

    /// Get a config items value, either true or false
    pub fn get_config(&self, key: &str) -> Option<i64> {
        match self.config.get(key) {
            Some(s) => Some(*s),
            None => None,
        }
    }

    /// Set a config items value to an integer
    pub fn set_config(&mut self, key: &str, value: i64) -> bool {
        match self.config.get_mut(key) {
            Some(s) => {
                *s = value;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_set_and_get_config_item() {
        let mut config = super::Config::new();
        assert_eq!(config.get_config("BackPressure"), Some(20));
        assert_eq!(config.set_config("BackPressure", 0), true);
        assert_eq!(config.get_config("BackPressure"), Some(0));
    }

    #[test]
    fn check_get_non_existent_config_item() {
        let config = super::Config::new();
        assert_eq!(config.get_config("NotExists"), None);
    }

    #[test]
    fn check_set_non_existent_config_item() {
        let mut config = super::Config::new();
        assert_eq!(config.set_config("NotExists", 2), false);
    }
}
