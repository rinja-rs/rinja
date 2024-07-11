#![doc(hidden)]

use std::cell::Cell;
use std::fmt;
use std::iter::{Enumerate, Peekable};

pub struct TemplateLoop<I>
where
    I: Iterator,
{
    iter: Peekable<Enumerate<I>>,
}

impl<I> TemplateLoop<I>
where
    I: Iterator,
{
    #[inline]
    pub fn new(iter: I) -> Self {
        TemplateLoop {
            iter: iter.enumerate().peekable(),
        }
    }
}

impl<I> Iterator for TemplateLoop<I>
where
    I: Iterator,
{
    type Item = (<I as Iterator>::Item, LoopItem);

    #[inline]
    fn next(&mut self) -> Option<(<I as Iterator>::Item, LoopItem)> {
        self.iter.next().map(|(index, item)| {
            (
                item,
                LoopItem {
                    index,
                    first: index == 0,
                    last: self.iter.peek().is_none(),
                },
            )
        })
    }
}

#[derive(Copy, Clone)]
pub struct LoopItem {
    pub index: usize,
    pub first: bool,
    pub last: bool,
}

pub struct FmtCell<F> {
    func: Cell<Option<F>>,
    err: Cell<Option<crate::Error>>,
}

impl<F> FmtCell<F>
where
    F: for<'a, 'b> FnOnce(&'a mut fmt::Formatter<'b>) -> crate::Result<()>,
{
    #[inline]
    pub fn new(f: F) -> Self {
        Self {
            func: Cell::new(Some(f)),
            err: Cell::new(None),
        }
    }

    #[inline]
    pub fn take_err(&self) -> crate::Result<()> {
        Err(self.err.take().unwrap_or(crate::Error::Fmt))
    }
}

impl<F> fmt::Display for FmtCell<F>
where
    F: for<'a, 'b> FnOnce(&'a mut fmt::Formatter<'b>) -> crate::Result<()>,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(func) = self.func.take() {
            if let Err(err) = func(f) {
                self.err.set(Some(err));
                return Err(fmt::Error);
            }
        }
        Ok(())
    }
}

pub fn get_primitive_value<T: PrimitiveType>(value: &T) -> T::Value {
    value.get()
}

pub trait PrimitiveType: Copy {
    type Value: Copy;

    fn get(self) -> Self::Value;
}

impl<T: PrimitiveType> PrimitiveType for &T {
    type Value = T::Value;

    #[inline]
    fn get(self) -> Self::Value {
        T::get(*self)
    }
}

macro_rules! primitive_type {
    ($($ty:ty),* $(,)?) => {$(
        impl PrimitiveType for $ty {
            type Value = $ty;

            #[inline]
            fn get(self) -> Self::Value {
                self
            }
        }
    )*};
}

primitive_type! {
    bool,
    f32, f64,
    i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
}
