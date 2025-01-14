#![allow(clippy::new_without_default)]

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::any::Any;
use core::borrow::Borrow;
use core::cmp::Ord;

/// Used to store values of any type at runtime.
///
/// It is implemented on [`HashMap`] (when the `std` feature is enabled),
/// on [`BTreeMap`] (when the `alloc` feature is enabled) and on slices.
pub trait Values {
    /// Add a new entry.
    #[cfg_attr(feature = "alloc", doc = "```")]
    #[cfg_attr(not(feature = "alloc"), doc = "```ignore")]
    /// use core::any::Any;
    /// use alloc::collections::BTreeMap;
    /// use rinja::Values;
    ///
    /// let mut v: BTreeMap<String, Box<dyn Any>> = BTreeMap::default();
    ///
    /// v.add_value("Hello".into(), 12u8);
    /// ```
    fn add_value(&mut self, key: impl Borrow<str>, value: impl Any);

    /// Retrieve an entry added with the [`add`][Self::add] method.
    #[cfg_attr(feature = "alloc", doc = "```")]
    #[cfg_attr(not(feature = "alloc"), doc = "```ignore")]
    /// use core::any::Any;
    /// use alloc::collections::BTreeMap;
    /// use rinja::Values;
    ///
    /// let mut v: BTreeMap<String, Box<dyn Any>> = BTreeMap::default();
    ///
    /// v.add_value("Hello".into(), 12u8);
    /// assert_eq!(v.get_value::<u8>("Hell", Err(ValueError::NotPresent)));
    /// assert_eq!(v.get_value::<usize>("Hello", Err(ValueError::WrongType)));
    /// assert_eq!(v.get_value::<u8>("Hello", Ok(12u8)));
    /// ```
    fn get_value<T: 'static>(&self, key: &(impl Borrow<str> + ?Sized)) -> Result<&T, ValueError>;
}

/// Returned by [`Values::get`].
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValueError {
    /// Returned in cases there is no value with the given key.
    NotPresent,
    /// Returned if the given value doesn't have the provided type.
    WrongType,
}

#[cfg(feature = "std")]
impl<K: for<'a> From<&'a str> + Borrow<str> + Ord + core::hash::Hash + ?Sized> Values
    for std::collections::HashMap<K, Box<dyn Any>>
{
    fn add_value(&mut self, key: impl Borrow<str>, value: impl Any) {
        let key: &str = key.borrow();
        self.insert(key.into(), Box::new(value));
    }

    fn get_value<T: 'static>(&self, key: &(impl Borrow<str> + ?Sized)) -> Result<&T, ValueError> {
        let key: &str = key.borrow();
        let Some(value) = self.get(key.into()) else {
            return Err(ValueError::NotPresent);
        };
        match value.downcast_ref::<T>() {
            Some(v) => Ok(v),
            None => Err(ValueError::WrongType),
        }
    }
}

#[cfg(feature = "alloc")]
impl<K: for<'a> From<&'a str> + Ord> Values for alloc::collections::BTreeMap<K, Box<dyn Any>> {
    fn add_value(&mut self, key: impl Borrow<str>, value: impl Any) {
        let key: &str = key.borrow();
        self.insert(key.into(), Box::new(value));
    }

    fn get_value<T: 'static>(&self, key: &(impl Borrow<str> + ?Sized)) -> Result<&T, ValueError> {
        let key: &str = key.borrow();
        let Some(value) = self.get(&key.into()) else {
            return Err(ValueError::NotPresent);
        };
        match value.downcast_ref::<T>() {
            Some(v) => Ok(v),
            None => Err(ValueError::WrongType),
        }
    }
}

impl<'a, K: Borrow<str> + PartialEq> Values for &'a [(K, &'a dyn Any)] {
    /// This method does nothing on this type.
    fn add_value(&mut self, _key: impl Borrow<str>, _value: impl Any) {}
    fn get_value<T: 'static>(&self, key: &(impl Borrow<str> + ?Sized)) -> Result<&T, ValueError> {
        let key = key.borrow();
        let Some(value) = self.iter().find_map(|(i_key, value)| {
            if key == i_key.borrow() {
                Some(value)
            } else {
                None
            }
        }) else {
            return Err(ValueError::NotPresent);
        };
        match value.downcast_ref::<T>() {
            Some(v) => Ok(v),
            None => Err(ValueError::WrongType),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn values_on_hashmap() {
        use std::collections::HashMap;
        use std::string::String;

        let mut values: HashMap<String, Box<dyn Any>> = HashMap::new();
        values.insert("a".into(), Box::new(10u32));
        values.insert("c".into(), Box::new("blam"));
        values.add_value("b", 12u32);

        assert_eq!(values.get_value::<u32>("a"), Ok(&10u32));
        assert_eq!(values.get_value::<&str>("c"), Ok(&"blam"));
        assert_eq!(values.get_value::<u8>("a"), Err(ValueError::WrongType));
        assert_eq!(values.get_value::<u8>("d"), Err(ValueError::NotPresent));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn values_on_btreemap() {
        use alloc::collections::BTreeMap;
        use std::string::String;

        let mut values: BTreeMap<String, Box<dyn Any>> = BTreeMap::new();
        values.insert("a".into(), Box::new(10u32));
        values.insert("c".into(), Box::new("blam"));
        values.add_value("b", 12u32);

        assert_eq!(values.get_value::<u32>("a"), Ok(&10u32));
        assert_eq!(values.get_value::<&str>("c"), Ok(&"blam"));
        assert_eq!(values.get_value::<u8>("a"), Err(ValueError::WrongType));
        assert_eq!(values.get_value::<u8>("d"), Err(ValueError::NotPresent));
    }

    #[test]
    fn values_on_slice() {
        let values: &[(&str, &dyn Any)] = &[("a", &12u32), ("c", &"blam")];

        assert_eq!(values.get_value::<u32>("a"), Ok(&12u32));
        assert_eq!(values.get_value::<&str>("c"), Ok(&"blam"));
        assert_eq!(values.get_value::<u8>("a"), Err(ValueError::WrongType));
        assert_eq!(values.get_value::<u8>("d"), Err(ValueError::NotPresent));
    }
}
