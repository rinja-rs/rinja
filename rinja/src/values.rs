#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(feature = "alloc")]
use alloc::string::String;
use core::any::Any;
#[cfg(feature = "std")]
use std::collections::HashMap;

// This whole code is to allow the crate to compile if both `alloc` and `std` features
// are disabled. The type will do nothing.
#[cfg(not(any(feature = "std", feature = "alloc")))]
struct HashMap;
#[cfg(not(any(feature = "std", feature = "alloc")))]
impl HashMap {
    fn new() -> Self {
        Self
    }
    fn insert(&mut self, _key: String, _value: impl Any) {}
    fn get(&self, _key: &str) -> Option<V> {
        None
    }
}

/// Used to store values of any type at runtime.
///
/// If the `std` feature is enabled, it internally uses the [`HashMap`] type, if the `alloc`
/// feature is enabled, it uses the [`BTreeMap`](alloc::collections::BTreeMap) type, otherwise
/// it uses an empty type and does nothing.
#[cfg(feature = "alloc")]
pub struct Values(HashMap<String, Box<dyn Any>>);
#[cfg(not(feature = "alloc"))]
pub struct Values(HashMap<String, ()>);

/// Returned by [`Values::get`].
#[derive(Debug)]
pub enum ValueError {
    /// Returned in cases there is no value with the given key.
    NotPresent,
    /// Returned if the given value doesn't have the provided type.
    WrongType,
}

impl Values {
    /// Constructor for `Values`.
    ///
    /// For a complete example, take a look at [`Values::get`].
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Add a new entry.
    ///
    /// ```
    /// let mut v = Values::new();
    ///
    /// v.add("Hello".into(), 12u8);
    /// ```
    pub fn add(&mut self, key: impl AsRef<str>, value: impl Any) {
        self.0.insert(key.as_ref().into(), Box::new(value));
    }

    /// ```
    /// let mut v = Values::new();
    ///
    /// v.add("Hello".into(), 12u8);
    /// assert_eq!(v.get::<u8>("Hell", Err(ValueError::NotPresent)));
    /// assert_eq!(v.get::<usize>("Hello", Err(ValueError::WrongType)));
    /// assert_eq!(v.get::<u8>("Hello", Ok(12u8)));
    /// ```
    pub fn get<T: 'static>(&self, key: impl AsRef<str>) -> Result<&T, ValueError> {
        let key: &str = key.as_ref();
        let Some(value) = self.0.get(key) else {
            return Err(ValueError::NotPresent);
        };
        match value.downcast_ref::<T>() {
            Some(v) => Ok(v),
            None => Err(ValueError::WrongType),
        }
    }
}
