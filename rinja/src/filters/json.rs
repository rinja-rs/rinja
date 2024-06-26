use std::convert::Infallible;
use std::{fmt, io, str};

use serde::Serialize;
use serde_json::ser::{to_writer, PrettyFormatter, Serializer};

/// Serialize to JSON (requires `json` feature)
///
/// The generated string does not contain ampersands `&`, chevrons `< >`, or apostrophes `'`.
/// To use it in a `<script>` you can combine it with the safe filter:
///
/// ``` html
/// <script>
/// var data = {{data|json|safe}};
/// </script>
/// ```
///
/// To use it in HTML attributes, you can either use it in quotation marks `"{{data|json}}"` as is,
/// or in apostrophes with the (optional) safe filter `'{{data|json|safe}}'`.
/// In HTML texts the output of e.g. `<pre>{{data|json|safe}}</pre>` is safe, too.
#[inline]
pub fn json(value: impl Serialize) -> Result<impl fmt::Display, Infallible> {
    Ok(ToJson { value })
}

/// Serialize to formatted/prettified JSON (requires `json` feature)
///
/// This filter works the same as [`json()`], but it formats the data for human readability.
/// It has an additional "indent" argument, which can either be an integer how many spaces to use
/// for indentation (capped to 16 characters), or a string (e.g. `"\u{A0}\u{A0}"` for two
/// non-breaking spaces).
///
/// ### Note
///
/// In rinja's template language, this filter is called `|json`, too. The right function is
/// automatically selected depending on whether an `indent` argument was provided or not.
#[inline]
pub fn json_pretty(
    value: impl Serialize,
    indent: impl AsIndent,
) -> Result<impl fmt::Display, Infallible> {
    Ok(ToJsonPretty { value, indent })
}

#[derive(Debug, Clone)]
struct ToJson<S> {
    value: S,
}

#[derive(Debug, Clone)]
struct ToJsonPretty<S, I> {
    value: S,
    indent: I,
}

pub trait AsIndent {
    fn as_indent(&self) -> &str;
}

impl AsIndent for str {
    #[inline]
    fn as_indent(&self) -> &str {
        self
    }
}

impl AsIndent for String {
    #[inline]
    fn as_indent(&self) -> &str {
        self
    }
}

impl AsIndent for usize {
    #[inline]
    fn as_indent(&self) -> &str {
        const MAX_SPACES: usize = 16;
        const SPACES: &str = match str::from_utf8(&[b' '; MAX_SPACES]) {
            Ok(spaces) => spaces,
            Err(_) => panic!(),
        };

        &SPACES[..(*self).min(SPACES.len())]
    }
}

impl<T: AsIndent + ?Sized> AsIndent for &T {
    #[inline]
    fn as_indent(&self) -> &str {
        T::as_indent(self)
    }
}

impl<T: AsIndent + ?Sized> AsIndent for Box<T> {
    #[inline]
    fn as_indent(&self) -> &str {
        T::as_indent(self.as_ref())
    }
}

impl<T: AsIndent + ToOwned + ?Sized> AsIndent for std::borrow::Cow<'_, T> {
    #[inline]
    fn as_indent(&self) -> &str {
        T::as_indent(self.as_ref())
    }
}

impl<T: AsIndent + ?Sized> AsIndent for std::rc::Rc<T> {
    #[inline]
    fn as_indent(&self) -> &str {
        T::as_indent(self.as_ref())
    }
}

impl<T: AsIndent + ?Sized> AsIndent for std::sync::Arc<T> {
    #[inline]
    fn as_indent(&self) -> &str {
        T::as_indent(self.as_ref())
    }
}

impl<S: Serialize> fmt::Display for ToJson<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        to_writer(JsonWriter(f), &self.value).map_err(|_| fmt::Error)
    }
}

impl<S: Serialize, I: AsIndent> fmt::Display for ToJsonPretty<S, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = self.indent.as_indent();
        let formatter = PrettyFormatter::with_indent(indent.as_bytes());
        let mut serializer = Serializer::with_formatter(JsonWriter(f), formatter);
        self.value
            .serialize(&mut serializer)
            .map_err(|_| fmt::Error)
    }
}

struct JsonWriter<'a, 'b: 'a>(&'a mut fmt::Formatter<'b>);

impl io::Write for JsonWriter<'_, '_> {
    #[inline]
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    #[inline]
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        write(self.0, bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    let mut last = 0;
    for (index, byte) in bytes.iter().enumerate() {
        let escaped = match byte {
            b'&' => Some(br"\u0026"),
            b'\'' => Some(br"\u0027"),
            b'<' => Some(br"\u003c"),
            b'>' => Some(br"\u003e"),
            _ => None,
        };
        if let Some(escaped) = escaped {
            f.write_str(unsafe { str::from_utf8_unchecked(&bytes[last..index]) })?;
            f.write_str(unsafe { str::from_utf8_unchecked(escaped) })?;
            last = index + 1;
        }
    }
    f.write_str(unsafe { str::from_utf8_unchecked(&bytes[last..]) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ugly() {
        assert_eq!(json(true).unwrap().to_string(), "true");
        assert_eq!(json("foo").unwrap().to_string(), r#""foo""#);
        assert_eq!(json(true).unwrap().to_string(), "true");
        assert_eq!(json("foo").unwrap().to_string(), r#""foo""#);
        assert_eq!(
            json("<script>").unwrap().to_string(),
            r#""\u003cscript\u003e""#
        );
        assert_eq!(
            json(vec!["foo", "bar"]).unwrap().to_string(),
            r#"["foo","bar"]"#
        );
    }

    #[test]
    fn test_pretty() {
        assert_eq!(json_pretty(true, "").unwrap().to_string(), "true");
        assert_eq!(
            json_pretty("<script>", "").unwrap().to_string(),
            r#""\u003cscript\u003e""#
        );
        assert_eq!(
            json_pretty(vec!["foo", "bar"], "").unwrap().to_string(),
            r#"[
"foo",
"bar"
]"#
        );
        assert_eq!(
            json_pretty(vec!["foo", "bar"], 2).unwrap().to_string(),
            r#"[
  "foo",
  "bar"
]"#
        );
        assert_eq!(
            json_pretty(vec!["foo", "bar"], "————").unwrap().to_string(),
            r#"[
————"foo",
————"bar"
]"#
        );
    }
}
