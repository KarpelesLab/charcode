//! Serialization round-trips through the encoding's name, and deserialization
//! accepts any of the standard's labels.

#![cfg(all(feature = "serde", feature = "std"))]

use charcode::{Encoding, ISO_8859_1, UTF_8, WINDOWS_31J};

#[test]
fn serializes_as_its_name() {
    assert_eq!(serde_json::to_string(UTF_8).unwrap(), "\"UTF-8\"");
    assert_eq!(
        serde_json::to_string(WINDOWS_31J).unwrap(),
        "\"windows-31j\""
    );
}

#[test]
fn deserializes_from_any_label() {
    let parse = |s: &str| serde_json::from_str::<&'static Encoding>(s).unwrap();
    assert_eq!(parse("\"UTF-8\""), UTF_8);
    assert_eq!(parse("\"utf8\""), UTF_8);
    // Deserialization uses the general lookup, so `latin1` is ISO-8859-1 and
    // not the superset the WHATWG Encoding Standard resolves it to.
    assert_eq!(parse("\" latin1 \""), ISO_8859_1);
    assert!(serde_json::from_str::<&'static Encoding>("\"nope\"").is_err());
}

#[test]
fn round_trips_every_encoding() {
    for &encoding in Encoding::all() {
        let json = serde_json::to_string(encoding).unwrap();
        assert_eq!(
            serde_json::from_str::<&'static Encoding>(&json).unwrap(),
            encoding
        );
    }
}
