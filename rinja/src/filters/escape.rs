use std::convert::Infallible;
use std::fmt::{self, Display, Formatter, Write};
use std::str;

/// Marks a string (or other `Display` type) as safe
///
/// Use this if you want to allow markup in an expression, or if you know
/// that the expression's contents don't need to be escaped.
///
/// Rinja will automatically insert the first (`Escaper`) argument,
/// so this filter only takes a single argument of any type that implements
/// `Display`.
#[inline]
pub fn safe(text: impl fmt::Display, escaper: impl Escaper) -> Result<impl Display, Infallible> {
    let _ = escaper; // it should not be part of the interface that the `escaper` is unused
    Ok(text)
}

/// Escapes strings according to the escape mode.
///
/// Rinja will automatically insert the first (`Escaper`) argument,
/// so this filter only takes a single argument of any type that implements
/// `Display`.
///
/// It is possible to optionally specify an escaper other than the default for
/// the template's extension, like `{{ val|escape("txt") }}`.
#[inline]
pub fn escape(text: impl fmt::Display, escaper: impl Escaper) -> Result<impl Display, Infallible> {
    struct EscapeDisplay<T, E>(T, E);
    struct EscapeWriter<W, E>(W, E);

    impl<T: fmt::Display, E: Escaper> fmt::Display for EscapeDisplay<T, E> {
        #[inline]
        fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
            write!(EscapeWriter(fmt, self.1), "{}", &self.0)
        }
    }

    impl<W: Write, E: Escaper> Write for EscapeWriter<W, E> {
        #[inline]
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.1.write_escaped_str(&mut self.0, s)
        }

        #[inline]
        fn write_char(&mut self, c: char) -> fmt::Result {
            self.1.write_escaped_char(&mut self.0, c)
        }
    }

    Ok(EscapeDisplay(text, escaper))
}

/// Alias for [`escape()`]
#[inline]
pub fn e(text: impl fmt::Display, escaper: impl Escaper) -> Result<impl Display, Infallible> {
    escape(text, escaper)
}

/// Escape characters in a safe way for HTML texts and attributes
///
/// * `<` => `&lt;`
/// * `>` => `&gt;`
/// * `&` => `&amp;`
/// * `"` => `&quot;`
/// * `'` => `&#x27;`
#[derive(Debug, Clone, Copy, Default)]
pub struct Html;

impl Escaper for Html {
    fn write_escaped_str<W: Write>(&self, mut fmt: W, string: &str) -> fmt::Result {
        let mut last = 0;
        for (index, byte) in string.bytes().enumerate() {
            const MIN_CHAR: u8 = b'"';
            const MAX_CHAR: u8 = b'>';
            const TABLE: [Option<&&str>; (MAX_CHAR - MIN_CHAR + 1) as usize] = {
                let mut table = [None; (MAX_CHAR - MIN_CHAR + 1) as usize];
                table[(b'<' - MIN_CHAR) as usize] = Some(&"&lt;");
                table[(b'>' - MIN_CHAR) as usize] = Some(&"&gt;");
                table[(b'&' - MIN_CHAR) as usize] = Some(&"&amp;");
                table[(b'"' - MIN_CHAR) as usize] = Some(&"&quot;");
                table[(b'\'' - MIN_CHAR) as usize] = Some(&"&#x27;");
                table
            };

            let escaped = match byte {
                MIN_CHAR..=MAX_CHAR => TABLE[(byte - MIN_CHAR) as usize],
                _ => None,
            };
            if let Some(escaped) = escaped {
                fmt.write_str(&string[last..index])?;
                fmt.write_str(escaped)?;
                last = index + 1;
            }
        }
        fmt.write_str(&string[last..])
    }

    fn write_escaped_char<W: Write>(&self, mut fmt: W, c: char) -> fmt::Result {
        fmt.write_str(match (c.is_ascii(), c as u8) {
            (true, b'<') => "&lt;",
            (true, b'>') => "&gt;",
            (true, b'&') => "&amp;",
            (true, b'"') => "&quot;",
            (true, b'\'') => "&#x27;",
            _ => return fmt.write_char(c),
        })
    }
}

/// Don't escape the input but return in verbatim
#[derive(Debug, Clone, Copy, Default)]
pub struct Text;

impl Escaper for Text {
    #[inline]
    fn write_escaped_str<W: Write>(&self, mut fmt: W, string: &str) -> fmt::Result {
        fmt.write_str(string)
    }

    #[inline]
    fn write_escaped_char<W: Write>(&self, mut fmt: W, c: char) -> fmt::Result {
        fmt.write_char(c)
    }
}

pub trait Escaper: Copy {
    fn write_escaped_str<W: Write>(&self, fmt: W, string: &str) -> fmt::Result;

    #[inline]
    fn write_escaped_char<W: Write>(&self, fmt: W, c: char) -> fmt::Result {
        self.write_escaped_str(fmt, c.encode_utf8(&mut [0; 4]))
    }
}

#[test]
fn test_escape() {
    assert_eq!(escape("", Html).unwrap().to_string(), "");
    assert_eq!(escape("<&>", Html).unwrap().to_string(), "&lt;&amp;&gt;");
    assert_eq!(escape("bla&", Html).unwrap().to_string(), "bla&amp;");
    assert_eq!(escape("<foo", Html).unwrap().to_string(), "&lt;foo");
    assert_eq!(escape("bla&h", Html).unwrap().to_string(), "bla&amp;h");

    assert_eq!(escape("", Text).unwrap().to_string(), "");
    assert_eq!(escape("<&>", Text).unwrap().to_string(), "<&>");
    assert_eq!(escape("bla&", Text).unwrap().to_string(), "bla&");
    assert_eq!(escape("<foo", Text).unwrap().to_string(), "<foo");
    assert_eq!(escape("bla&h", Text).unwrap().to_string(), "bla&h");
}
