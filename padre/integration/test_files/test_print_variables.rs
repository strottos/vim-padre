use std::io;

fn main() -> io::Result<()> {
    let a: i32 = 42;
    let b = &a;
    let a: f32 = 42.1;
    let a: bool = true;
    let a = "TEST";
    let b = &a;
    let a = "TEST".to_string();
    let b = &a;
    let a: Vec<String> = vec!("TEST1".to_string(),
                              "TEST2".to_string(),
                              "TEST3".to_string());

    Ok(())
}
