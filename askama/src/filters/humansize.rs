use core::convert::Infallible;
use core::fmt;
use core::mem::MaybeUninit;
use core::str::from_utf8_unchecked;

use super::FastWritable;

/// Returns adequate string representation (in KB, ..) of number of bytes
///
/// ## Example
/// ```
/// # use askama::Template;
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

impl fmt::Display for FilesizeFormatFilter {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(self.write_into(f)?)
    }
}

impl FastWritable for FilesizeFormatFilter {
    fn write_into<W: fmt::Write + ?Sized>(&self, dest: &mut W) -> crate::Result<()> {
        if self.0 < 1e3 {
            (self.0 as u32).write_into(dest)?;
            Ok(dest.write_str(" B")?)
        } else if let Some((prefix, factor)) = SI_PREFIXES
            .iter()
            .copied()
            .find_map(|(prefix_factor, max)| (self.0 < max).then_some(prefix_factor))
        {
            // u32 is big enough to hold the number 999_999
            let scaled = (self.0 * factor) as u32;
            (scaled / 100).write_into(dest)?;
            format_frac(&mut MaybeUninit::uninit(), prefix, scaled).write_into(dest)
        } else {
            too_big(self.0, dest)
        }
    }
}

/// Formats `buffer` to contain the decimal point, decimal places and unit
fn format_frac(buffer: &mut MaybeUninit<[u8; 8]>, prefix: u8, scaled: u32) -> &str {
    // LLVM generates better byte code for register sized buffers, so we add some NULs
    let buffer = buffer.write(*b"..0 kB\0\0");
    buffer[4] = prefix;

    let frac = scaled % 100;
    let buffer = if frac == 0 {
        &buffer[3..6]
    } else if frac % 10 == 0 {
        // the decimal separator '.' is already contained in buffer[1]
        buffer[2] = b'0' + (frac / 10) as u8;
        &buffer[1..6]
    } else {
        // the decimal separator '.' is already contained in buffer[0]
        buffer[1] = b'0' + (frac / 10) as u8;
        buffer[2] = b'0' + (frac % 10) as u8;
        &buffer[0..6]
    };
    // SAFETY: we know that the buffer contains only ASCII data
    unsafe { from_utf8_unchecked(buffer) }
}

#[cold]
fn too_big<W: fmt::Write + ?Sized>(value: f32, dest: &mut W) -> crate::Result<()> {
    // the value exceeds 999 QB, so we omit the decimal places
    Ok(write!(dest, "{:.0} QB", value / 1e30)?)
}

/// `((si_prefix, factor), limit)`, the factor is offset by 10**2 to account for 2 decimal places
const SI_PREFIXES: &[((u8, f32), f32)] = &[
    ((b'k', 1e-1), 1e6),
    ((b'M', 1e-4), 1e9),
    ((b'G', 1e-7), 1e12),
    ((b'T', 1e-10), 1e15),
    ((b'P', 1e-13), 1e18),
    ((b'E', 1e-16), 1e21),
    ((b'Z', 1e-19), 1e24),
    ((b'Y', 1e-22), 1e27),
    ((b'R', 1e-25), 1e30),
    ((b'Q', 1e-28), 1e33),
];

#[test]
#[cfg(feature = "alloc")]
fn test_filesizeformat() {
    use alloc::string::ToString;

    assert_eq!(filesizeformat(0.).unwrap().to_string(), "0 B");
    assert_eq!(filesizeformat(999.).unwrap().to_string(), "999 B");
    assert_eq!(filesizeformat(1000.).unwrap().to_string(), "1 kB");
    assert_eq!(filesizeformat(1023.).unwrap().to_string(), "1.02 kB");
    assert_eq!(filesizeformat(1024.).unwrap().to_string(), "1.02 kB");
    assert_eq!(filesizeformat(1100.).unwrap().to_string(), "1.1 kB");
    assert_eq!(filesizeformat(9_499_014.).unwrap().to_string(), "9.49 MB");
    assert_eq!(
        filesizeformat(954_548_589.2).unwrap().to_string(),
        "954.54 MB"
    );
}
