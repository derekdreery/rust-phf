//! A set of builders to generate Rust source for PHF data structures at
//! compile time.
//!
//! The provided builders are intended to be used in a Cargo build script to
//! generate a Rust source file that will be included in a library at build
//! time.
//!
//! # Examples
//!
//! build.rs
//!
//! ```rust,no_run
//! extern crate phf_codegen;
//!
//! use std::env;
//! use std::fs::File;
//! use std::io::{BufWriter, Write};
//! use std::path::Path;
//!
//! fn main() {
//!     let path = Path::new(&env::var("OUT_DIR").unwrap()).join("codegen.rs");
//!     let mut file = BufWriter::new(File::create(&path).unwrap());
//!
//! write!(&mut file, "static KEYWORDS: phf::Map<&'static str, Keyword> =
//! ").unwrap();
//!     phf_codegen::Map::new()
//!         .entry("loop", "Keyword::Loop")
//!         .entry("continue", "Keyword::Continue")
//!         .entry("break", "Keyword::Break")
//!         .entry("fn", "Keyword::Fn")
//!         .entry("extern", "Keyword::Extern")
//!         .build(&mut file)
//!         .unwrap();
//!     write!(&mut file, ";\n").unwrap();
//! }
//! ```
//!
//! lib.rs
//!
//! ```ignore
//! extern crate phf;
//!
//! #[derive(Clone)]
//! enum Keyword {
//!     Loop,
//!     Continue,
//!     Break,
//!     Fn,
//!     Extern,
//! }
//!
//! include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
//!
//! pub fn parse_keyword(keyword: &str) -> Option<Keyword> {
//!     KEYWORDS.get(keyword).cloned()
//! }
//! ```
//!
//! # Note
//!
//! The compiler's stack will overflow when processing extremely long method
//! chains (500+ calls). When generating large PHF data structures, consider
//! looping over the entries or making each call a separate statement:
//!
//! ```rust
//! let entries = [("hello", "1"), ("world", "2")];
//!
//! let mut builder = phf_codegen::Map::new();
//! for &(key, value) in &entries {
//!     builder.entry(key, value);
//! }
//! // ...
//! ```
//!
//! ```rust
//! let mut builder = phf_codegen::Map::new();
//! builder.entry("hello", "1");
//! builder.entry("world", "2");
//! // ...
//! ```
#![doc(html_root_url = "https://docs.rs/phf_codegen/0.7.20")]
extern crate phf_generator;
extern crate phf_shared;

use std::ascii;
use std::collections::HashSet;
use std::fmt::{self, Write as FmtWrite};
use std::hash::Hash;
use std::io;
use std::io::prelude::*;

pub trait Source {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result;
}

impl<'a, T> Source for &'a T
where
    T: ?Sized + Source,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(fmt)
    }
}

impl Source for str {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("\"")?;
        for raw in self.chars() {
            for escaped in raw.escape_default() {
                fmt.write_char(escaped as char)?;
            }
        }
        fmt.write_str("\"")
    }
}

impl Source for [u8] {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("b\"")?;
        for raw in self {
            for escaped in ascii::escape_default(*raw) {
                fmt.write_char(escaped as char)?;
            }
        }
        fmt.write_str("\" as &[u8]")
    }
}

struct Displayify<T>(T);

impl<T> fmt::Display for Displayify<T>
where
    T: Source,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        Source::fmt(&self.0, fmt)
    }
}

/// A builder for the `phf::Map` type.
pub struct Map<K> {
    keys: Vec<K>,
    values: Vec<String>,
    path: String,
}

impl<K: AsRef<[u8]> + Hash + Eq + Source> Map<K> {
    /// Creates a new `phf::Map` builder.
    pub fn new() -> Map<K> {
        Map {
            keys: vec![],
            values: vec![],
            path: "::phf".to_string(),
        }
    }

    /// Set the path to the `phf` crate from the global namespace
    pub fn phf_path(&mut self, path: &str) -> &mut Map<K> {
        self.path = path.to_owned();
        self
    }

    /// Adds an entry to the builder.
    ///
    /// `value` will be written exactly as provided in the constructed source.
    pub fn entry(&mut self, key: K, value: &str) -> &mut Map<K> {
        self.keys.push(key);
        self.values.push(value.to_owned());
        self
    }

    /// Constructs a `phf::Map`, outputting Rust source to the provided writer.
    ///
    /// # Panics
    ///
    /// Panics if there are any duplicate keys.
    pub fn build<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let mut set = HashSet::new();
        for key in &self.keys {
            if !set.insert(key) {
                panic!("duplicate key `{}`", Displayify(key));
            }
        }

        let state = phf_generator::generate_hash(&self.keys);

        try!(write!(
            w,
            "{}::Map {{
    key: {},
    disps: &[",
            self.path, state.key
        ));
        for &(d1, d2) in &state.disps {
            try!(write!(
                w,
                "
        ({}, {}),",
                d1, d2
            ));
        }
        try!(write!(
            w,
            "
    ],
    entries: &[",
        ));
        for &idx in &state.map {
            try!(write!(
                w,
                "
        ({}, {}),",
                Displayify(&self.keys[idx]),
                &self.values[idx]
            ));
        }
        write!(
            w,
            "
    ],
}}"
        )
    }
}

/// A builder for the `phf::Set` type.
pub struct Set<T> {
    map: Map<T>,
}

impl<T: AsRef<[u8]> + Hash + Eq + Source> Set<T> {
    /// Constructs a new `phf::Set` builder.
    pub fn new() -> Set<T> {
        Set { map: Map::new() }
    }

    /// Set the path to the `phf` crate from the global namespace
    pub fn phf_path(&mut self, path: &str) -> &mut Set<T> {
        self.map.phf_path(path);
        self
    }

    /// Adds an entry to the builder.
    pub fn entry(&mut self, entry: T) -> &mut Set<T> {
        self.map.entry(entry, "()");
        self
    }

    /// Constructs a `phf::Set`, outputting Rust source to the provided writer.
    ///
    /// # Panics
    ///
    /// Panics if there are any duplicate entries.
    pub fn build<W: Write>(&self, w: &mut W) -> io::Result<()> {
        try!(write!(w, "{}::Set {{ map: ", self.map.path));
        try!(self.map.build(w));
        write!(w, " }}")
    }
}
