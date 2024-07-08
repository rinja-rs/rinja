use std::convert::Infallible;
use std::fmt::{self, Display, Formatter, Write};
use std::num::NonZeroU8;
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
/// * `"` => `&#34;`
/// * `&` => `&#38;`
/// * `'` => `&#39;`
/// * `<` => `&#60;`
/// * `>` => `&#62;`
#[derive(Debug, Clone, Copy, Default)]
pub struct Html;

impl Escaper for Html {
    fn write_escaped_str<W: Write>(&self, mut fmt: W, string: &str) -> fmt::Result {
        let mut escaped_buf = *b"&#__;";
        let mut last = 0;

        for (index, byte) in string.bytes().enumerate() {
            const MIN_CHAR: u8 = b'"';
            const MAX_CHAR: u8 = b'>';

            struct Table {
                _align: [usize; 0],
                lookup: [Option<[NonZeroU8; 2]>; (MAX_CHAR - MIN_CHAR + 1) as usize],
            }

            const TABLE: Table = {
                const fn n(c: u8) -> Option<[NonZeroU8; 2]> {
                    let n0 = match NonZeroU8::new(c / 10 + b'0') {
                        Some(n) => n,
                        None => panic!(),
                    };
                    let n1 = match NonZeroU8::new(c % 10 + b'0') {
                        Some(n) => n,
                        None => panic!(),
                    };
                    Some([n0, n1])
                }

                let mut table = Table {
                    _align: [],
                    lookup: [None; (MAX_CHAR - MIN_CHAR + 1) as usize],
                };

                table.lookup[(b'"' - MIN_CHAR) as usize] = n(b'"');
                table.lookup[(b'&' - MIN_CHAR) as usize] = n(b'&');
                table.lookup[(b'\'' - MIN_CHAR) as usize] = n(b'\'');
                table.lookup[(b'<' - MIN_CHAR) as usize] = n(b'<');
                table.lookup[(b'>' - MIN_CHAR) as usize] = n(b'>');
                table
            };

            let escaped = match byte {
                MIN_CHAR..=MAX_CHAR => TABLE.lookup[(byte - MIN_CHAR) as usize],
                _ => None,
            };
            if let Some(escaped) = escaped {
                escaped_buf[2] = escaped[0].get();
                escaped_buf[3] = escaped[1].get();
                fmt.write_str(&string[last..index])?;
                fmt.write_str(unsafe { std::str::from_utf8_unchecked(escaped_buf.as_slice()) })?;
                last = index + 1;
            }
        }
        fmt.write_str(&string[last..])
    }

    fn write_escaped_char<W: Write>(&self, mut fmt: W, c: char) -> fmt::Result {
        fmt.write_str(match (c.is_ascii(), c as u8) {
            (true, b'"') => "&#34;",
            (true, b'&') => "&#38;",
            (true, b'\'') => "&#39;",
            (true, b'<') => "&#60;",
            (true, b'>') => "&#62;",
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
    assert_eq!(escape("<&>", Html).unwrap().to_string(), "&#60;&#38;&#62;");
    assert_eq!(escape("bla&", Html).unwrap().to_string(), "bla&#38;");
    assert_eq!(escape("<foo", Html).unwrap().to_string(), "&#60;foo");
    assert_eq!(escape("bla&h", Html).unwrap().to_string(), "bla&#38;h");

    assert_eq!(escape("", Text).unwrap().to_string(), "");
    assert_eq!(escape("<&>", Text).unwrap().to_string(), "<&>");
    assert_eq!(escape("bla&", Text).unwrap().to_string(), "bla&");
    assert_eq!(escape("<foo", Text).unwrap().to_string(), "<foo");
    assert_eq!(escape("bla&h", Text).unwrap().to_string(), "bla&h");
}
