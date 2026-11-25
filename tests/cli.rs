use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU32, Ordering},
};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn run(csv: &str, flags: [&str; 3]) -> (bool, String, Option<String>) {
    let dir = std::env::temp_dir().join(format!(
        "csv-to-json-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("input.csv"), csv).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_csv-to-json"))
        .args(["input.csv", ","])
        .args(flags)
        .current_dir(&dir)
        .output()
        .unwrap();
    let json = fs::read_to_string(dir.join("rows.json")).ok();
    fs::remove_dir_all(dir).unwrap();
    (
        output.status.success(),
        String::from_utf8(output.stdout)
            .unwrap()
            .trim_end()
            .to_string(),
        json,
    )
}

#[test]
fn converts_multiline_fields_and_orders_numeric_headers() {
    let (ok, output, json) = run(
        "\u{feff}10,2,value\r\nx,y,\"one\r\ntwo \"\"three\"\"\"\r\n",
        ["CRLF", "header", "skip"],
    );
    assert!(ok);
    assert_eq!(output, "1");
    assert_eq!(
        json.unwrap(),
        r#"[{"2":"y","10":"x","value":"one\r\ntwo \"three\""}]"#
    );
}

#[test]
fn distinguishes_blank_lines_from_empty_fields() {
    let text = "first\n\n\"\"\n\"two\n\nlines\"\n";
    for (blank_mode, expected) in [
        ("skip", r#"[["first"],[""],["two\n\nlines"]]"#),
        ("keep", r#"[["first"],[""],[""],["two\n\nlines"]]"#),
    ] {
        let (ok, _, json) = run(text, ["LF", "arrays", blank_mode]);
        assert!(ok);
        assert_eq!(json.unwrap(), expected);
    }
}

#[test]
fn reports_bad_width_before_writing_a_document() {
    let (ok, _, json) = run("a,b\nx\n", ["LF", "header", "skip"]);
    assert!(!ok);
    assert!(json.is_none());
}

#[test]
fn repeated_header_uses_last_column_and_preserves_key_order() {
    let (ok, _, json) = run(
        "5,name,tag,name,1\nx,first,extra,last,y\n",
        ["LF", "header", "skip"],
    );
    assert!(ok);
    assert_eq!(
        json.unwrap(),
        r#"[{"1":"y","5":"x","name":"last","tag":"extra"}]"#
    );
}
