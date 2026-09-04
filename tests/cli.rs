//! End-to-end tests for the `charcode` command-line tool.
//!
//! These drive the real binary, so they cover argument handling, the streaming
//! loop over stdin, and the exit status a script would branch on.

#![cfg(feature = "cli")]

use std::io::{self, Write};
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_charcode"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary was built");
    let mut pipe = child.stdin.take().expect("stdin is piped");
    match pipe.write_all(stdin) {
        Ok(()) => {}
        // The child is entitled to exit before reading its input — rejecting
        // its arguments is exactly that — so losing the race is not a failure.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
        Err(e) => panic!("writing to charcode {args:?} failed: {e}"),
    }
    // Closing the pipe is what tells the child the input has ended.
    drop(pipe);
    child.wait_with_output().expect("the child exits")
}

fn stdout(args: &[&str], stdin: &[u8]) -> Vec<u8> {
    let output = run(args, stdin);
    assert!(
        output.status.success(),
        "charcode {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn converts_between_encodings() {
    assert_eq!(
        stdout(&["-f", "latin1", "-t", "utf-8"], b"caf\xE9"),
        "café".as_bytes()
    );
    assert_eq!(
        stdout(&["-f", "utf-8", "-t", "latin1"], "café".as_bytes()),
        b"caf\xE9"
    );
    assert_eq!(
        stdout(&["-f", "shift_jis"], b"\x93\xFA\x96\x7B\x8C\xEA"),
        "日本語".as_bytes()
    );
    // A stateful target encoding gets its trailing escape.
    assert_eq!(
        stdout(&["-t", "iso-2022-jp"], "日".as_bytes()),
        b"\x1B$B\x46\x7C\x1B(B"
    );
}

#[test]
fn defaults_are_utf8_in_and_out() {
    assert_eq!(stdout(&[], "unchanged".as_bytes()), b"unchanged");
}

#[test]
fn a_byte_order_mark_overrides_the_named_encoding() {
    assert_eq!(
        stdout(&["-f", "windows-1252"], b"\xEF\xBB\xBFcaf\xC3\xA9"),
        "café".as_bytes()
    );
}

#[test]
fn malformed_input_fails_by_default() {
    let output = run(&["-f", "utf-8"], b"ok then \xFF");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty() || output.stdout == b"ok then ");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("malformed"), "{stderr}");
    assert!(stderr.contains("offset 8"), "{stderr}");
}

#[test]
fn unmappable_characters_fail_by_default() {
    let output = run(&["-t", "windows-1252"], "一".as_bytes());
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("U+4E00"), "{stderr}");
    assert!(stderr.contains("windows-1252"), "{stderr}");
}

#[test]
fn dash_c_omits_and_reports_a_lossy_conversion() {
    // As with iconv, -c still exits non-zero: the output is not faithful.
    let output = run(&["-c", "-t", "windows-1252"], b"a\xFFb\xE4\xB8\x80c");
    assert_eq!(output.stdout, b"abc");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("1 malformed byte sequence"), "{stderr}");
    assert!(stderr.contains("1 character(s)"), "{stderr}");

    // -s keeps the summary quiet without changing the output or the status.
    let output = run(&["-cs", "-t", "windows-1252"], b"a\xFFb\xE4\xB8\x80c");
    assert_eq!(output.stdout, b"abc");
    assert!(output.stderr.is_empty());
    assert!(!output.status.success());
}

#[test]
fn iconv_ignore_suffix_is_accepted() {
    let output = run(&["-t", "ascii//IGNORE"], "一X".as_bytes());
    assert_eq!(output.stdout, b"X");
    assert!(!output.status.success());
}

#[test]
fn files_are_converted_as_one_stream() {
    let dir = std::env::temp_dir().join(format!("charcode-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let (a, b) = (dir.join("a.bin"), dir.join("b.bin"));
    // The two bytes of one Shift_JIS character, split across two files.
    std::fs::write(&a, b"\x93").expect("write a");
    std::fs::write(&b, b"\xFA").expect("write b");
    let args = ["-f", "shift_jis", a.to_str().unwrap(), b.to_str().unwrap()];
    assert_eq!(stdout(&args, b""), "日".as_bytes());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn output_can_go_to_a_file() {
    let path = std::env::temp_dir().join(format!("charcode-out-{}.txt", std::process::id()));
    let args = ["-t", "koi8-r", "-o", path.to_str().unwrap()];
    assert!(stdout(&args, "Привет".as_bytes()).is_empty());
    assert_eq!(
        std::fs::read(&path).expect("output file"),
        b"\xF0\xD2\xC9\xD7\xC5\xD4"
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn listings_cover_the_whole_standard() {
    // Only the standard's own encodings are counted exactly; a build with the
    // charsets outside it listed more.
    const EXTRAS: bool = cfg!(any(
        feature = "dos",
        feature = "ebcdic",
        feature = "mac",
        feature = "misc"
    ));

    let encodings = stdout(&["--list"], b"");
    let encodings = String::from_utf8(encodings).expect("UTF-8");
    // 40 from the standard, plus ISO-8859-1 and US-ASCII, which it has no
    // room for.
    if EXTRAS {
        assert!(encodings.lines().count() > 42);
    } else {
        assert_eq!(encodings.lines().count(), 42);
    }
    assert!(encodings.lines().any(|l| l == "windows-1252"));

    let labels = stdout(&["--list-labels"], b"");
    let labels = String::from_utf8(labels).expect("UTF-8");
    // The general listing drops the 52 labels the standard reassigns and adds
    // the ones ISO-8859-1 and US-ASCII bring.
    assert!(labels.lines().count() > 180);
    // The listing is the honest one: `latin1` names ISO-8859-1 here, whatever
    // the WHATWG Encoding Standard resolves it to for the web.
    assert!(labels.lines().any(|l| l == "latin1\tISO-8859-1"));
    assert!(labels.lines().any(|l| l == "windows-1252\twindows-1252"));
    // The listing is sorted by label.
    let mut sorted: Vec<&str> = labels.lines().collect();
    sorted.sort_unstable();
    assert_eq!(sorted, labels.lines().collect::<Vec<_>>());
}

#[test]
fn bad_usage_is_diagnosed_and_exits_non_zero() {
    // The input is deliberately larger than a pipe buffer.  A run rejected for
    // its arguments exits before reading any of it, so the write gets
    // `BrokenPipe`; feeding it a payload this size makes that happen every
    // time rather than whenever the scheduler feels like it.
    let unread = vec![b'x'; 1 << 20];
    for args in [
        &["-f", "bogus"][..],
        &["--nope"],
        &["-t", "utf-8//NOPE"],
        &["--bom=sideways"],
    ] {
        let output = run(args, &unread);
        assert!(!output.status.success(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
    // The replacement encoding is refused with an explanation rather than used.
    let output = run(&["-f", "hz-gb-2312"], b"x");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("replacement"));
}

#[test]
fn help_and_version_succeed() {
    let help = stdout(&["--help"], b"");
    assert!(String::from_utf8_lossy(&help).starts_with("Usage: charcode"));
    let version = stdout(&["--version"], b"");
    assert_eq!(
        String::from_utf8_lossy(&version).trim(),
        format!("charcode {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn input_larger_than_one_read_chunk_is_streamed() {
    // Bigger than the 64 KiB read buffer, so the conversion state has to carry
    // across chunk boundaries.
    let text = "日本語".repeat(20_000);
    let converted = stdout(&["-t", "euc-jp"], text.as_bytes());
    let back = stdout(&["-f", "euc-jp"], &converted);
    assert_eq!(back, text.as_bytes());
}
