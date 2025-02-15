use std::env::args_os;
use std::fs::OpenOptions;
use std::io::{Read, Write, stdin};
use std::path::{Path, PathBuf};

use fuzz::{DisplayTargets, TARGETS};

fn main() -> Result<(), Error> {
    let mut args = args_os().fuse();
    let exe = args.next().map(PathBuf::from);
    let name = args.next().and_then(|s| s.into_string().ok());
    let src = args.next().map(PathBuf::from);
    let dest = args.next().map(PathBuf::from);
    let empty = args.next().map(|_| ());

    let (Some(name), Some(src), None) = (name, src, empty) else {
        return Err(Error::Usage(exe));
    };

    let scenario_builder = TARGETS
        .iter()
        .find_map(|&(scenario, func)| (scenario == name).then_some(func))
        .ok_or(Error::Target(name))?;

    let mut data = Vec::new();
    if src == Path::new("-") {
        stdin().read_to_end(&mut data).map_err(Error::Stdin)?;
    } else {
        match OpenOptions::new().read(true).open(Path::new(&src)) {
            Ok(mut f) => {
                f.read_to_end(&mut data)
                    .map_err(|err| Error::Read(err, src))?;
            }
            Err(err) => return Err(Error::Open(err, src)),
        }
    }

    let scenario = scenario_builder(&data).map_err(Error::Build)?;
    if let Some(dest) = dest {
        if dest == Path::new("-") {
            println!("{scenario}");
        } else {
            let mut f = match OpenOptions::new().write(true).create_new(true).open(&dest) {
                Ok(f) => f,
                Err(err) => return Err(Error::DestOpen(err, dest)),
            };
            writeln!(f, "{scenario}").map_err(|err| Error::DestWrite(err, dest))?;
        }
    } else {
        eprintln!("{scenario:#?}");
        scenario.run().map_err(Error::Run)?;
        println!("Success.");
    }
    Ok(())
}

#[derive(thiserror::Error, pretty_error_debug::Debug)]
enum Error {
    #[error(
        "wrong arguments supplied\n\
        Usage: {} <target> <src> [<dest>]\n\
        * <target>   {DisplayTargets}\n\
        * <src>      failed scenario (supply '-' to from from STDIN)\n\
        * <dest>     write a #[test] to this file (optional; supply '-' to write to STDOUT)",
        .0.as_deref().unwrap_or(Path::new("askama_fuzzing")).display(),
    )]
    Usage(Option<PathBuf>),
    #[error("unknown fuzzing target {:?}\nImplemented targets: {DisplayTargets}", .0)]
    Target(String),
    #[error("could not read standard input")]
    Stdin(#[source] std::io::Error),
    #[error("could not open input file {:?}", .1.display())]
    Open(#[source] std::io::Error, PathBuf),
    #[error("could not read opened input file {:?}", .1.display())]
    Read(#[source] std::io::Error, PathBuf),
    #[error("could not build scenario")]
    Build(#[source] arbitrary::Error),
    #[error("could not run scenario")]
    Run(#[source] Box<dyn std::error::Error + Send + 'static>),
    #[error("could could not create destination file {:?} for writing", .1.display())]
    DestOpen(#[source] std::io::Error, PathBuf),
    #[error("could could not write to opened destination file {:?}", .1.display())]
    DestWrite(#[source] std::io::Error, PathBuf),
}
