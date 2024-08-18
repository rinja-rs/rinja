macro_rules! this_file {
    ($(crate :: $mod:ident :: $ty:ident;)*) => {
        use std::fmt;

        use arbitrary::{Arbitrary, Unstructured};

        #[derive(Debug)]
        pub enum Scenario<'a> {
            $($ty(crate::$mod::Scenario<'a>),)*
        }

        impl<'a> super::Scenario<'a> for Scenario<'a> {
            type RunError = RunError;

            fn new(data: &'a [u8]) -> Result<Self, arbitrary::Error> {
                let mut data = Unstructured::new(data);
                let target = Arbitrary::arbitrary(&mut data)?;
                let data = Arbitrary::arbitrary_take_rest(data)?;

                match target {
                    $(Target::$ty => Ok(Self::$ty(crate::$mod::Scenario::new(data)?)),)*
                }
            }

            fn run(&self) -> Result<(), Self::RunError> {
                match self {
                    $(Self::$ty(scenario) => scenario.run().map_err(RunError::$ty),)*
                }
            }
        }

        #[derive(Arbitrary)]
        enum Target {
            $($ty,)*
        }

        pub enum RunError {
            $($ty(<crate::$mod::Scenario<'static> as crate::Scenario<'static>>::RunError),)*
        }

        impl fmt::Display for RunError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$ty(err) => err.fmt(f),)*
                }
            }
        }

        impl fmt::Debug for RunError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$ty(err) => err.fmt(f),)*
                }
            }
        }

        impl std::error::Error for RunError {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self  {
                    $(RunError::$ty(err) => Some(err),)*
                }
            }
        }
    };
}

this_file! {
    crate::html::Html;
    crate::parser::Parser;
}
