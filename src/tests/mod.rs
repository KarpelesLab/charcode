//! The crate's own tests.
//!
//! [`buffers`] exercises the streaming API through fixed-size buffers, so it
//! runs in every configuration, including the one without an allocator.  The
//! rest need `alloc` for their expectations.

mod buffers;

#[cfg(all(
    feature = "alloc",
    any(feature = "dos", feature = "ebcdic", feature = "mac", feature = "misc")
))]
mod extra;
#[cfg(feature = "alloc")]
mod indexes;
#[cfg(feature = "alloc")]
mod round_trip;
#[cfg(feature = "alloc")]
mod streaming;
