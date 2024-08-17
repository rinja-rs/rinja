# Rinja Parser Fuzzer

First install `cargo-fuzz` and rust-nightly (once):

```sh
cargo install cargo-fuzz
rustup install nightly
```

Then execute in this folder:

```sh
RUST_BACKTRACE=1 nice cargo +nightly fuzz run fuzz
```

The execution won't stop, but continue until you kill it with ctrl+c.
Or until it finds a panic.
If the execution found a panic, then a file with the input scenario is written, e.g.
`fuzz/artifacts/fuzz/crash-4184…`.
To get more information about the failed scenario, run or debug this command with the given path:

```sh
cargo run -- fuzz/artifacts/fuzz/crash-4184…
``` 

Find more information about fuzzing here:

* `cargo fuzz help run`
* <https://rust-fuzz.github.io/book/cargo-fuzz.html>
