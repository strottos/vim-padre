/// Various utility for use in PADRE
use std::env;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Get an unused port on the local system and return it. This port
/// can subsequently be used.
pub fn get_unused_localhost_port() -> u16 {
    let listener = TcpListener::bind(format!("127.0.0.1:0")).unwrap();
    listener.local_addr().unwrap().port()
}

/// Find out if a file is a binary executable (either ELF or Mach-O
/// executable).
pub fn file_is_binary_executable(cmd: &str) -> bool {
    let output = get_file_type(cmd);

    if output.contains("ELF")
        || (output.contains("Mach-O") && output.to_ascii_lowercase().contains("executable"))
    {
        true
    } else {
        false
    }
}

/// Find out if a file is a text file (either ASCII or UTF-8).
pub fn file_is_text(cmd: &str) -> bool {
    let output = get_file_type(cmd);

    if output.contains("ASCII") || output.contains("UTF-8") {
        true
    } else {
        false
    }
}

/// Find out if a file is a binary executable (either ELF or Mach-O
/// executable). It will try to find the file first, failing that
/// it will try to find it in the path and failing that it will
/// return the empty string.
pub fn get_file_full_path(cmd: &str) -> String {
    let cmd_full_path_buf = env::var_os("PATH")
        .and_then(|paths| {
            env::split_paths(&paths)
                .filter_map(|dir| {
                    let cmd_full_path = dir.join(&cmd);
                    if cmd_full_path.is_file() {
                        Some(cmd_full_path)
                    } else {
                        None
                    }
                })
                .next()
        })
        .unwrap_or(PathBuf::from(cmd));
    String::from(cmd_full_path_buf.as_path().to_str().unwrap())
}

/// Return true if the path specified exists.
pub fn file_exists(path: &str) -> bool {
    if !Path::new(path).exists() {
        false
    } else {
        true
    }
}

fn get_file_type(cmd: &str) -> String {
    let output = Command::new("file")
        .arg("-L") // Follow symlinks
        .arg(cmd)
        .output()
        .expect(&format!("Can't run file on {} to find file type", cmd));

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::net::TcpListener;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    fn get_test_path_env_var() -> String {
        format!(
            "{}:{}:/bin:/usr/bin",
            Path::new("./test_files")
                .canonicalize()
                .expect("Cannot find test_files directory")
                .as_path()
                .to_str()
                .unwrap(),
            Path::new("./integration/test_files")
                .canonicalize()
                .expect("Cannot find test_files directory")
                .as_path()
                .to_str()
                .unwrap(),
        )
    }

    #[test]
    fn find_and_use_unused_port() {
        let port = super::get_unused_localhost_port();
        thread::sleep(Duration::new(1, 0));
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[test]
    fn is_file_executable() {
        assert_eq!(true, super::file_is_binary_executable("./test_files/node"));
        assert_eq!(
            false,
            super::file_is_binary_executable("./test_files/test_node.js")
        );
    }

    #[test]
    fn is_file_text() {
        assert_eq!(false, super::file_is_text("./test_files/node"));
        assert_eq!(true, super::file_is_text("./test_files/test_node.js"));
    }

    #[test]
    fn test_file_exists() {
        assert_eq!(true, super::file_exists("./test_files/node"));
    }

    #[test]
    fn test_file_not_exists() {
        assert_eq!(false, super::file_exists("./test_files/not_exists"));
    }

    #[test]
    fn test_getting_files_full_path_for_absolute_path() {
        let old_path = env::var("PATH").unwrap();
        let path_var = get_test_path_env_var();
        env::set_var("PATH", &path_var);

        assert_eq!(
            "./test_files/node".to_string(),
            super::get_file_full_path("./test_files/node")
        );

        env::set_var("PATH", old_path);
    }

    #[test]
    fn test_getting_files_full_path() {
        let old_path = env::var("PATH").unwrap();
        let path_var = get_test_path_env_var();
        env::set_var("PATH", &path_var);

        let test_files_path_raw = String::from("./test_files/node");
        let test_files_path = Path::new(&test_files_path_raw)
            .canonicalize()
            .expect("Cannot find test_files directory");

        assert_eq!(
            test_files_path.as_path().to_str().unwrap(),
            super::get_file_full_path("node")
        );

        env::set_var("PATH", old_path);
    }

    #[test]
    fn test_getting_files_full_path_when_not_exists() {
        assert_eq!(
            "file_surely_doesnt_exist".to_string(),
            super::get_file_full_path("file_surely_doesnt_exist")
        );
    }
}
