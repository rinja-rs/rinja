use std::convert::Infallible;
use std::fmt;

/// Returns adequate string representation (in KB, ..) of number of bytes
///
/// ## Example
/// ```
/// # use rinja::Template;
/// #[derive(Template)]
/// #[template(
///     source = "Filesize: {{ size_in_bytes|filesizeformat }}.",
///     ext = "html"
/// )]
/// struct Example {
///     size_in_bytes: u64,
/// }
///
/// let tmpl = Example { size_in_bytes: 1_234_567 };
/// assert_eq!(tmpl.to_string(),  "Filesize: 1.23 MB.");
/// ```
#[inline]
pub fn filesizeformat(b: f32) -> Result<FilesizeFormatFilter, Infallible> {
    Ok(FilesizeFormatFilter(b))
}

#[derive(Debug, Clone, Copy)]
pub struct FilesizeFormatFilter(f32);

struct NbAndDecimal(u32);
impl NbAndDecimal {
    fn new(nb: f32) -> Self {
        // `nb` will never be bigger than 999_999 so we're fine with usize.
        Self(nb as _)
    }
}
impl fmt::Display for NbAndDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sub = self.0 % 1_000 / 10;
        if sub != 0 {
            if sub < 10 {
                f.write_fmt(format_args!("{}.0{sub}", self.0 / 1_000))
            } else {
                f.write_fmt(format_args!("{}.{sub}", self.0 / 1_000))
            }
        } else {
            f.write_fmt(format_args!("{}", self.0 / 1_000))
        }
    }
}

impl fmt::Display for FilesizeFormatFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 < 1e3 {
            return f.write_fmt(format_args!("{} B", self.0));
        }
        for (unit, max, divider) in [
            (" KB", 1e6, 1.),
            (" MB", 1e9, 1e3),
            (" GB", 1e12, 1e6),
            (" TB", 1e15, 1e9),
            (" PB", 1e18, 1e12),
            (" EB", 1e21, 1e15),
            (" ZB", 1e24, 1e18),
        ] {
            if self.0 < max {
                f.write_fmt(format_args!("{}", NbAndDecimal::new(self.0 / divider)))?;
                return f.write_str(unit);
            }
        }
        f.write_fmt(format_args!("{} YB", NbAndDecimal::new(self.0 / 1e21)))
    }
}

#[test]
#[allow(clippy::needless_borrows_for_generic_args)]
fn test_filesizeformat() {
    assert_eq!(filesizeformat(0.).unwrap().to_string(), "0 B");
    assert_eq!(filesizeformat(999.).unwrap().to_string(), "999 B");
    assert_eq!(filesizeformat(1000.).unwrap().to_string(), "1 KB");
    assert_eq!(filesizeformat(1023.).unwrap().to_string(), "1.02 KB");
    assert_eq!(filesizeformat(1024.).unwrap().to_string(), "1.02 KB");
    assert_eq!(filesizeformat(9_499_014.).unwrap().to_string(), "9.49 MB");
    assert_eq!(
        filesizeformat(954_548_589.2).unwrap().to_string(),
        "954.54 MB"
    );
}
