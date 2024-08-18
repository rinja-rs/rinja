pub mod all;
pub mod filters;
pub mod html;
pub mod parser;

use std::error::Error;
use std::fmt;

pub const TARGETS: &[(&str, TargetBuilder)] = &[
    ("all", |data| NamedTarget::new::<all::Scenario>(data)),
    ("filters", |data| {
        NamedTarget::new::<filters::Scenario>(data)
    }),
    ("html", |data| NamedTarget::new::<html::Scenario>(data)),
    ("parser", |data| NamedTarget::new::<parser::Scenario>(data)),
];

pub type TargetBuilder = for<'a> fn(&'a [u8]) -> Result<NamedTarget<'a>, arbitrary::Error>;

pub trait Scenario<'a>: fmt::Debug + Sized {
    type RunError: Error + Send + 'static;

    fn fuzz(data: &'a [u8]) -> Result<(), FuzzError<Self::RunError>> {
        Self::new(data)
            .map_err(FuzzError::New)?
            .run()
            .map_err(FuzzError::Run)
    }

    fn new(data: &'a [u8]) -> Result<Self, arbitrary::Error>;
    fn run(&self) -> Result<(), Self::RunError>;
}

#[derive(Debug, thiserror::Error)]
pub enum FuzzError<RunError: Error + Send + 'static> {
    #[error("could not build scenario")]
    New(#[source] arbitrary::Error),
    #[error("could not run scenario")]
    Run(#[source] RunError),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayTargets;

impl fmt::Display for DisplayTargets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (idx, &(name, _)) in TARGETS.iter().enumerate() {
            match idx {
                0 => write!(f, "{name}")?,
                _ => write!(f, "|{name}")?,
            };
        }
        Ok(())
    }
}

pub struct NamedTarget<'a>(Box<dyn RunScenario<'a> + 'a>);

impl NamedTarget<'_> {
    #[inline]
    pub fn run(&self) -> Result<(), Box<dyn Error + Send + 'static>> {
        self.0.run()
    }
}

impl fmt::Debug for NamedTarget<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

impl<'a> NamedTarget<'a> {
    #[inline]
    fn new<S: Scenario<'a> + 'a>(data: &'a [u8]) -> Result<Self, arbitrary::Error> {
        Ok(Self(Box::new(S::new(data)?)))
    }
}

trait RunScenario<'a> {
    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    fn run(&self) -> Result<(), Box<dyn Error + Send + 'static>>;
}

impl<'a, T: Scenario<'a>> RunScenario<'a> for T {
    #[inline]
    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt(f)
    }

    #[inline]
    fn run(&self) -> Result<(), Box<dyn Error + Send + 'static>> {
        match self.run() {
            Ok(()) => Ok(()),
            Err(err) => Err(Box::new(err)),
        }
    }
}
