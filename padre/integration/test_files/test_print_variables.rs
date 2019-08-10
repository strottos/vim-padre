use std::io;

struct TestStructInner<'a> {
    a: String,
    b: &'a str,
}

struct TestStruct<'a> {
    a: bool,
    b: i32,
    c: Vec<String>,
    d: TestStructInner<'a>,
}

fn main() -> io::Result<()> {
    let a: i32 = 42;
    let b = &a;
    let a: f32 = 42.1;
    let a: bool = true;
    let a = "TEST";
    let b = &a;
    let a = "TEST".to_string();
    let b = &a;
    let a: Vec<String> = vec![
        "TEST1".to_string(),
        "TEST2".to_string(),
        "TEST3".to_string(),
    ];
    let a: TestStruct = TestStruct {
        a: true,
        b: 42,
        c: vec!["TEST1".to_string()],
        d: TestStructInner {
            a: "TEST_INNER".to_string(),
            b: "TESTING",
        },
    };

    Ok(())
}
