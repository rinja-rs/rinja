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
