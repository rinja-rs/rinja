use std::fmt;
use std::num::NonZeroU8;

#[allow(unused)]
pub(crate) fn write_escaped_str(mut fmt: impl fmt::Write, string: &str) -> fmt::Result {
    let mut escaped_buf = *b"&#__;";
    let mut last = 0;

    for (index, byte) in string.bytes().enumerate() {
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

#[allow(unused)]
pub(crate) fn write_escaped_char(mut fmt: impl fmt::Write, c: char) -> fmt::Result {
    fmt.write_str(match (c.is_ascii(), c as u8) {
        (true, b'"') => "&#34;",
        (true, b'&') => "&#38;",
        (true, b'\'') => "&#39;",
        (true, b'<') => "&#60;",
        (true, b'>') => "&#62;",
        _ => return fmt.write_char(c),
    })
}

const MIN_CHAR: u8 = b'"';
const MAX_CHAR: u8 = b'>';

struct Table {
    _align: [usize; 0],
    lookup: [Option<[NonZeroU8; 2]>; (MAX_CHAR - MIN_CHAR + 1) as usize],
}

const TABLE: Table = {
    const fn n(c: u8) -> Option<[NonZeroU8; 2]> {
        assert!(MIN_CHAR <= c && c <= MAX_CHAR);

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
