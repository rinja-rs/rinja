This crate embeds the source of `rinja_derive`, but is not a `proc_macro`.
This way we can more easily access the internals of the crate.

To run the benchmark, execute `cargo bench` in this folder, or
`cargo bench -p rinja_derive_standalone` in the project root.
