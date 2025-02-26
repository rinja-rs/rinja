# Askama Fuzzing

First install `cargo-fuzz` and rust-nightly (once):

```sh
cargo install cargo-fuzz
rustup install nightly
```

Then execute in this folder:

```sh
RUST_BACKTRACE=1 nice cargo +nightly fuzz run <fuzz_target>
```

`fuzz_target` is one out of `all`, `filters`, `html` or `parser`.

The execution won't stop, but continue until you kill it with ctrl+c.
Or until it finds a panic.
If the execution found a panic, then a file with the input scenario is written, e.g.
`fuzz/artifacts/parser/crash-b91ab…`.
To get more information about the failed scenario, run or debug this command with the given path:

```sh
cargo run -- <fuzz_target> fuzz/artifacts/parser/crash-b91ab…
``` 

Find more information about fuzzing here:

* `cargo fuzz help run`
* <https://rust-fuzz.github.io/book/cargo-fuzz.html>
