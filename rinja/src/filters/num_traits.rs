use num_traits::Signed;
use num_traits::cast::NumCast;

use crate::{Error, Result};

/// Absolute value
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use rinja::Template;
/// /// ```jinja
/// /// <div>{{ -2|abs }}</div>
/// /// ```
///
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example;
///
/// assert_eq!(
///     Example.to_string(),
///     "<div>2</div>"
/// );
/// # }
/// ```
pub fn abs<T: Signed>(number: T) -> Result<T> {
    Ok(number.abs())
}

/// Casts number to f64
pub fn into_f64<T: NumCast>(number: T) -> Result<f64> {
    number.to_f64().ok_or(Error::Fmt)
}

/// Casts number to isize
pub fn into_isize<T: NumCast>(number: T) -> Result<isize> {
    number.to_isize().ok_or(Error::Fmt)
}

#[test]
#[allow(clippy::float_cmp)]
fn test_abs() {
    assert_eq!(abs(1).unwrap(), 1);
    assert_eq!(abs(-1).unwrap(), 1);
    assert_eq!(abs(1.0).unwrap(), 1.0);
    assert_eq!(abs(-1.0).unwrap(), 1.0);
    assert_eq!(abs(1.0_f64).unwrap(), 1.0_f64);
    assert_eq!(abs(-1.0_f64).unwrap(), 1.0_f64);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_into_f64() {
    assert_eq!(into_f64(1).unwrap(), 1.0_f64);
    assert_eq!(into_f64(1.9).unwrap(), 1.9_f64);
    assert_eq!(into_f64(-1.9).unwrap(), -1.9_f64);
    assert_eq!(into_f64(f32::INFINITY).unwrap(), f64::INFINITY);
    assert_eq!(into_f64(-f32::INFINITY).unwrap(), -f64::INFINITY);
}

#[test]
fn test_into_isize() {
    assert_eq!(into_isize(1).unwrap(), 1_isize);
    assert_eq!(into_isize(1.9).unwrap(), 1_isize);
    assert_eq!(into_isize(-1.9).unwrap(), -1_isize);
    assert_eq!(into_isize(1.5_f64).unwrap(), 1_isize);
    assert_eq!(into_isize(-1.5_f64).unwrap(), -1_isize);
    match into_isize(f64::INFINITY) {
        Err(Error::Fmt) => {}
        _ => panic!("Should return error of type Err(Error::Fmt)"),
    };
}
