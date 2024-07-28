use std::{fmt, str};

#[cfg(not(target_arch = "x86_64"))]
#[allow(unused)]
pub(crate) fn write_escaped_str(mut fmt: impl fmt::Write, string: &str) -> fmt::Result {
    // Even though [`jetscii`] ships a generic implementation for unsupported platforms,
    // it is not well optimized for this case. This implementation should work well enough in
    // the meantime, until portable SIMD gets stabilized.

    // Instead of testing the platform, we could test the CPU features. But given that the needed
    // instruction set SSE 4.2 was introduced in 2008, that it has an 99.61 % availability rate
    // in Steam's June 2024 hardware survey, and is a prerequisite to run Windows 11, I don't
    // think we need to care.

    let mut escaped_buf = ESCAPED_BUF_INIT;
    let mut last = 0;

    for (index, byte) in string.bytes().enumerate() {
        let escaped = match byte {
            MIN_CHAR..=MAX_CHAR => TABLE.lookup[(byte - MIN_CHAR) as usize],
            _ => 0,
        };
        if escaped != 0 {
            [escaped_buf[2], escaped_buf[3]] = escaped.to_ne_bytes();
            write_str_if_nonempty(&mut fmt, &string[last..index])?;
            // SAFETY: the content of `escaped_buf` is pure ASCII
            fmt.write_str(unsafe {
                std::str::from_utf8_unchecked(&escaped_buf[..ESCAPED_BUF_LEN])
            })?;
            last = index + 1;
        }
    }
    write_str_if_nonempty(&mut fmt, &string[last..])
}

#[cfg(target_arch = "x86_64")]
#[allow(unused)]
pub(crate) fn write_escaped_str(mut fmt: impl fmt::Write, mut string: &str) -> fmt::Result {
    let jetscii = jetscii::bytes!(b'"', b'&', b'\'', b'<', b'>');

    let mut escaped_buf = ESCAPED_BUF_INIT;
    loop {
        if string.is_empty() {
            return Ok(());
        }

        let found = if string.len() >= 16 {
            // Only strings of at least 16 bytes can be escaped using SSE instructions.
            match jetscii.find(string.as_bytes()) {
                Some(index) => {
                    let escaped = TABLE.lookup[(string.as_bytes()[index] - MIN_CHAR) as usize];
                    Some((index, escaped))
                }
                None => None,
            }
        } else {
            // The small-string fallback of [`jetscii`] is quite slow, so we roll our own
            // implementation.
            string.as_bytes().iter().find_map(|byte: &u8| {
                let escaped = get_escaped(*byte)?;
                let index = (byte as *const u8 as usize) - (string.as_ptr() as usize);
                Some((index, escaped))
            })
        };
        let Some((index, escaped)) = found else {
            return fmt.write_str(string);
        };

        [escaped_buf[2], escaped_buf[3]] = escaped.to_ne_bytes();

        // SAFETY: index points at an ASCII char in `string`
        let front;
        (front, string) = unsafe {
            (
                string.get_unchecked(..index),
                string.get_unchecked(index + 1..),
            )
        };

        write_str_if_nonempty(&mut fmt, front)?;
        // SAFETY: the content of `escaped_buf` is pure ASCII
        fmt.write_str(unsafe { str::from_utf8_unchecked(&escaped_buf[..ESCAPED_BUF_LEN]) })?;
    }
}

#[allow(unused)]
pub(crate) fn write_escaped_char(mut fmt: impl fmt::Write, c: char) -> fmt::Result {
    if !c.is_ascii() {
        fmt.write_char(c)
    } else if let Some(escaped) = get_escaped(c as u8) {
        let mut escaped_buf = ESCAPED_BUF_INIT;
        [escaped_buf[2], escaped_buf[3]] = escaped.to_ne_bytes();
        // SAFETY: the content of `escaped_buf` is pure ASCII
        fmt.write_str(unsafe { str::from_utf8_unchecked(&escaped_buf[..ESCAPED_BUF_LEN]) })
    } else {
        // RATIONALE: `write_char(c)` gets optimized if it is known that `c.is_ascii()`
        fmt.write_char(c)
    }
}

#[inline(always)]
fn get_escaped(byte: u8) -> Option<u16> {
    let c = byte.wrapping_sub(MIN_CHAR);
    if (c < u32::BITS as u8) && (BITS & (1 << c as u32) != 0) {
        Some(TABLE.lookup[c as usize])
    } else {
        None
    }
}

#[inline(always)]
fn write_str_if_nonempty(output: &mut impl fmt::Write, input: &str) -> fmt::Result {
    if !input.is_empty() {
        output.write_str(input)
    } else {
        Ok(())
    }
}

/// List of characters that need HTML escaping, not necessarily in ordinal order.
/// Filling the [`TABLE`] and [`BITS`] constants will ensure that the range of lowest to hightest
/// codepoint wont exceed [`u32::BITS`] (=32) items.
const CHARS: &[u8] = br#""&'<>"#;

/// The character with the smallest codepoint that needs HTML escaping.
/// Both [`TABLE`] and [`BITS`] start at this value instead of `0`.
const MIN_CHAR: u8 = {
    let mut v = u8::MAX;
    let mut i = 0;
    while i < CHARS.len() {
        if v > CHARS[i] {
            v = CHARS[i];
        }
        i += 1;
    }
    v
};

#[allow(unused)]
const MAX_CHAR: u8 = {
    let mut v = u8::MIN;
    let mut i = 0;
    while i < CHARS.len() {
        if v < CHARS[i] {
            v = CHARS[i];
        }
        i += 1;
    }
    v
};

struct Table {
    _align: [usize; 0],
    lookup: [u16; u32::BITS as usize],
}

/// For characters that need HTML escaping, the codepoint formatted as decimal digits,
/// otherwise `b"\0\0"`. Starting at [`MIN_CHAR`].
const TABLE: Table = {
    let mut table = Table {
        _align: [],
        lookup: [0; u32::BITS as usize],
    };
    let mut i = 0;
    while i < CHARS.len() {
        let c = CHARS[i];
        let h = c / 10 + b'0';
        let l = c % 10 + b'0';
        table.lookup[(c - MIN_CHAR) as usize] = u16::from_ne_bytes([h, l]);
        i += 1;
    }
    table
};

/// A bitset of the characters that need escaping, starting at [`MIN_CHAR`]
const BITS: u32 = {
    let mut i = 0;
    let mut bits = 0;
    while i < CHARS.len() {
        bits |= 1 << (CHARS[i] - MIN_CHAR) as u32;
        i += 1;
    }
    bits
};

// RATIONALE: llvm generates better code if the buffer is register sized
const ESCAPED_BUF_INIT: [u8; 8] = *b"&#__;\0\0\0";
const ESCAPED_BUF_LEN: usize = b"&#__;".len();

#[test]
fn simple() {
    let mut buf = String::new();
    write_escaped_str(&mut buf, "<script>").unwrap();
    assert_eq!(buf, "&#60;script&#62;");

    buf.clear();
    write_escaped_str(&mut buf, "s<crip>t").unwrap();
    assert_eq!(buf, "s&#60;crip&#62;t");

    buf.clear();
    write_escaped_str(&mut buf, "s<cripcripcripcripcripcripcripcripcripcrip>t").unwrap();
    assert_eq!(buf, "s&#60;cripcripcripcripcripcripcripcripcripcrip&#62;t");
}
