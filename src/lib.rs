//! A library for the MBON file type
//!
//! mbon is a binary notation that is inspired by the NBT format.
//!
//! It is formed of a sequence of strongly typed values. Each made up of two
//! parts: a mark which defines the type and size of the data, followed by the
//! data. Marks can be different in size and so a single byte prefix is used to
//! differenciate between types.
//!
//! This format is self-describing which means that it is able to know if the
//! data is not formatted correctly or a different type was stored than what was
//! expected. Another feature of the self-describing nature of the format is
//! that you can skip values in the data without the need to parse the complete
//! item, e.g. A 1GB value can be easily skipped by only reading the mark.
//!
//! # Usage
//!
//! mbon is primarily used with the [crate::engine::Engine] which allows for
//! reading and writing data with a stream. The engine is capable of
//! reading/writing whole items or sections of items.
//!
//! # Features
//!
//! There are two primary features that mbon may be compiled with.
//!
//! * `sync` — Builds the library without any async code/dependencies
//! * `async-tokio` — Builds the library using [tokio]'s async library.
//!
//! These two features are mutually exclusive, so compiling with both `sync` and
//! `async-tokio` will cause a compiler error.
//!
//! ```toml
//! [dependencies]
//! mbon = { version = "0.3.0", features = ["async-tokio"] }
//! ```
//!
//! These docs are written assuming that the `async-tokio` feature was set, any
//! functions that are marked as async will not be with the `sync` feature.
//!
//! # Spec
//!
//! A specification of the mbon file format can be found at
//! [github.com/ttocsneb/mbon/blob/rewrite/spec/)](https://github.com/ttocsneb/mbon/blob/rewrite/spec/index.md).
//!

#[cfg(not(any(feature = "sync", feature = "async-tokio")))]
compile_error!("Feature \"sync\" or \"async-tokio\" is required");
#[cfg(all(feature = "sync", feature = "async-tokio"))]
compile_error!("Only one of \"sync\" or \"async-tokio\" can be active at a time");

#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod buffer;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub(crate) mod channel;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod concurrent;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod data;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod engine;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod errors;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod items;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod marks;
#[cfg(any(feature = "sync", feature = "async-tokio"))]
pub mod stream;
