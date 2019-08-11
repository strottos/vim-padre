//! Configuration
//!
//! Responsible for setting default config, reading configs and setting.
//!
//! The following config items can be set:
//!  - ErrorWhenQueued: Set to true if we should respond with an error when a request is already
//!         in the queue or false. Defaults to false.
//!  - HaltOnUnknownPosition: Set to true if we should halt when we reach an unknown position or
//!         false if we should carry on. Default to false.
//!  - StepInOnUnknownPosition: Set to true if we should step in when at an unknown position or
//!         false if we should step over until we reach a known position. Assumes
//!         HaltOnUnknownPosition is true or has no effect. Defaults to false.

use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref CONFIG: Mutex<HashMap<&'static str, bool>> = {
        let mut m = HashMap::new();
        m.insert("ErrorWhenQueued", false);
        m.insert("HaltOnUnknownPosition", false);
        m.insert("StepInOnUnknownPosition", false);
        Mutex::new(m)
    };
}

/// Get a config items value, either true or false
pub fn get_bool_config(cfg: &str) -> Option<bool> {
    match CONFIG.lock().unwrap().get(cfg) {
        Some(s) => Some(*s),
        None => None,
    }
}

/// Set a config items value, either true or false
pub fn set_bool_config(cfg: &str, value: bool) {
    let bad;
    match CONFIG.lock().unwrap().get_mut(cfg) {
        Some(s) => {
            *s = value;
            bad = false;
        },
        None => {
            bad = true;
        }
    }
    if bad {
        panic!("Trying to set config item that doesn't exist {}", cfg);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_get_default_config_item() {
        super::set_bool_config("ErrorWhenQueued", false);
        assert_eq!(super::get_bool_config("ErrorWhenQueued"), Some(false));
    }

    #[test]
    fn check_get_non_existent_config_item() {
        assert_eq!(super::get_bool_config("NotExists"), None);
    }

    #[test]
    fn check_set_config_item() {
        super::set_bool_config("ErrorWhenQueued", true);
        assert_eq!(super::get_bool_config("ErrorWhenQueued"), Some(true));
    }

    #[test]
    #[should_panic]
    fn check_set_non_existent_config_item() {
        super::set_bool_config("NotExists", true);
    }
}
