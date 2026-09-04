//! `serde` support, gated behind the `serde` feature.
//!
//! An encoding serializes as its name, and deserializes through
//! [`Encoding::for_label`], so any of the standard's labels is accepted.

use serde::de::{Error, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Encoding;

impl Serialize for Encoding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

struct EncodingVisitor;

impl Visitor<'_> for EncodingVisitor {
    type Value = &'static Encoding;

    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a label for an encoding in the WHATWG Encoding Standard")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
        Encoding::for_label(value.as_bytes())
            .ok_or_else(|| E::invalid_value(Unexpected::Str(value), &self))
    }
}

impl<'de> Deserialize<'de> for &'static Encoding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(EncodingVisitor)
    }
}
