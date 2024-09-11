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
#[cfg(feature = "serde_json")]
mod json;

#[cfg(feature = "humansize")]
pub use builtin::filesizeformat;
#[cfg(feature = "num-traits")]
pub use builtin::{abs, into_f64, into_isize};
pub use builtin::{
    capitalize, center, fmt, format, indent, join, linebreaks, linebreaksbr, lower, lowercase,
    paragraphbreaks, pluralize, title, trim, truncate, upper, uppercase, wordcount, PluralizeCount,
};
#[cfg(feature = "urlencode")]
pub use builtin::{urlencode, urlencode_strict};
pub use escape::{
    e, escape, safe, AutoEscape, AutoEscaper, Escaper, FastWritable, Html, HtmlSafe,
    HtmlSafeOutput, MaybeSafe, Safe, Text, Unsafe, Writable, WriteWritable,
};
#[cfg(feature = "serde_json")]
pub use json::{json, json_pretty, AsIndent};
