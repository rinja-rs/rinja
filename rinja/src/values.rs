#![allow(clippy::new_without_default)]

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(feature = "alloc")]
use alloc::string::String;
use core::any::Any;
#[cfg(feature = "std")]
use std::collections::HashMap;

/// Used to store values of any type at runtime.
///
/// If the `std` feature is enabled, it internally uses the [`HashMap`] type, if the `alloc`
/// feature is enabled, it uses the [`BTreeMap`](alloc::collections::BTreeMap) type, otherwise
/// it uses an empty type and does nothing.
#[derive(Default)]
pub struct Values(#[cfg(feature = "alloc")] HashMap<String, Box<dyn Any>>);

/// Returned by [`Values::get`].
#[derive(Debug)]
pub enum ValueError {
    /// Returned in cases there is no value with the given key.
    NotPresent,
    /// Returned if the given value doesn't have the provided type.
    WrongType,
}

impl Values {
    /// Add a new entry.
    ///
    /// ```
    /// let mut v = Values::default();
    ///
    /// v.add("Hello".into(), 12u8);
    /// ```
    pub fn add(&mut self, _key: impl AsRef<str>, _value: impl Any) {
        #[cfg(feature = "alloc")]
        self.0.insert(_key.as_ref().into(), Box::new(_value));
    }

    /// ```
    /// let mut v = Values::default();
    ///
    /// v.add("Hello".into(), 12u8);
    /// assert_eq!(v.get::<u8>("Hell", Err(ValueError::NotPresent)));
    /// assert_eq!(v.get::<usize>("Hello", Err(ValueError::WrongType)));
    /// assert_eq!(v.get::<u8>("Hello", Ok(12u8)));
    /// ```
    pub fn get<T: 'static>(&self, _key: impl AsRef<str>) -> Result<&T, ValueError> {
        #[cfg(feature = "alloc")]
        {
            let key: &str = _key.as_ref();
            let Some(value) = self.0.get(key) else {
                return Err(ValueError::NotPresent);
            };
            match value.downcast_ref::<T>() {
                Some(v) => Ok(v),
                None => Err(ValueError::WrongType),
            }
        }
        #[cfg(not(feature = "alloc"))]
        Err(ValueError::NotPresent)
    }
}
