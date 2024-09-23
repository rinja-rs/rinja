//! Module for built-in filter functions
//!
//! Contains all the built-in filter functions for use in templates.
//! You can define your own filters, as well.
//!
//! ## Note
//!
//! All **result types of any filter function** in this module is **subject to change** at any
//! point, and is **not indicated by as semver breaking** version bump.
//! The traits [`AutoEscape`] and [`WriteWritable`] are used by [`rinja_derive`]'s generated code
//! to work with all compatible types.

mod builtin;
mod escape;
#[cfg(feature = "humansize")]
mod humansize;
#[cfg(feature = "serde_json")]
mod json;
#[cfg(feature = "num-traits")]
mod num_traits;
#[cfg(feature = "urlencode")]
mod urlencode;

pub use builtin::{
    capitalize, center, fmt, format, indent, join, linebreaks, linebreaksbr, lower, lowercase,
    paragraphbreaks, pluralize, title, trim, truncate, upper, uppercase, wordcount, PluralizeCount,
};
pub use escape::{
    e, escape, safe, AutoEscape, AutoEscaper, Escaper, FastWritable, Html, HtmlSafe,
    HtmlSafeOutput, MaybeSafe, Safe, Text, Unsafe, Writable, WriteWritable,
};
#[cfg(feature = "humansize")]
pub use humansize::filesizeformat;
#[cfg(feature = "serde_json")]
pub use json::{json, json_pretty, AsIndent};
#[cfg(feature = "num-traits")]
pub use num_traits::{abs, into_f64, into_isize};
#[cfg(feature = "urlencode")]
pub use urlencode::{urlencode, urlencode_strict};
