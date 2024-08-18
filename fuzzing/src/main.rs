use std::env::args_os;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

use fuzz::{DisplayTargets, TARGETS};

fn main() -> Result<(), Error> {
    let mut args = args_os().fuse();
    let exe = args.next().map(PathBuf::from);
    let name = args.next().and_then(|s| s.into_string().ok());
    let path = args.next().map(PathBuf::from);
    let empty = args.next().map(|_| ());

    let (Some(name), Some(path), None) = (name, path, empty) else {
        return Err(Error::Usage(exe));
    };

    let scenario_builder = TARGETS
        .iter()
        .find_map(|&(scenario, func)| (scenario == name).then_some(func))
        .ok_or(Error::Target(name))?;

    let mut data = Vec::new();
    match OpenOptions::new().read(true).open(Path::new(&path)) {
        Ok(mut f) => {
            f.read_to_end(&mut data)
                .map_err(|err| Error::Read(err, path))?;
        }
        Err(err) => return Err(Error::Open(err, path)),
    };

    let scenario = scenario_builder(&data).map_err(Error::Build)?;
    eprintln!("{scenario:#?}");
    scenario.run().map_err(Error::Run)?;
    println!("Success.");

    Ok(())
}

#[derive(thiserror::Error, pretty_error_debug::Debug)]
enum Error {
    #[error(
        "wrong arguments supplied\nUsage: {} <{DisplayTargets}> <path>",
        .0.as_deref().unwrap_or(Path::new("rinja_fuzzing")).display(),
    )]
    Usage(Option<PathBuf>),
    #[error("unknown fuzzing target {:?}\nImplemented targets: {DisplayTargets}", .0)]
    Target(String),
    #[error("could not open input file {}", .1.display())]
    Open(#[source] std::io::Error, PathBuf),
    #[error("could not read opened input file {}", .1.display())]
    Read(#[source] std::io::Error, PathBuf),
    #[error("could not build scenario")]
    Build(#[source] arbitrary::Error),
    #[error("could not run scenario")]
    Run(#[source] Box<dyn std::error::Error + Send + 'static>),
}
