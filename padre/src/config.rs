//! Configuration
//!
//! Responsible for setting default config, reading configs and setting.
//!
//! The following config items can be set:
//!  - BackPressure: Set the backpressure of the queue to build up. 0 means it errors on any
//!    request when one is in progress. Defaults to 20.
//!  - UnknownPosition: Set to the following:
//!    0: if we should halt when we reach an unknown position
//!    1: if we should carry on stepping over until we reach a known position.
//!    2: if we should carry on stepping in until we reach a known position.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::notifier::{log_msg, LogLevel};

lazy_static! {
    static ref CONFIG: Mutex<HashMap<&'static str, i64>> = {
        let mut m = HashMap::new();
        m.insert("BackPressure", 20);
        m.insert("UnknownPosition", 0);
        Mutex::new(m)
    };
}

/// Get a config items value, either true or false
pub fn get_config(cfg: &str) -> Option<i64> {
    match CONFIG.lock().unwrap().get(cfg) {
        Some(s) => Some(*s),
        None => None,
    }
}

/// Set a config items value to an integer
pub fn set_config(cfg: &str, value: i64) {
    match CONFIG.lock().unwrap().get_mut(cfg) {
        Some(s) => {
            *s = value;
            return;
        }
        None => {
            log_msg(
                LogLevel::WARN,
                &format!("Couldn't set unfound config item: {}", cfg),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_set_and_get_config_item() {
        assert_eq!(super::get_config("BackPressure"), Some(20));
        super::set_config("BackPressure", 0);
        assert_eq!(super::get_config("BackPressure"), Some(0));
    }

    #[test]
    fn check_get_non_existent_config_item() {
        assert_eq!(super::get_config("NotExists"), None);
    }
}
