# Performance

## Slow Debug Recompilations

If you experience slow compile times when iterating with lots of templates,
you can compile Rinja's derive macros with a higher optimization level.
This can speed up recompilation times dramatically.

Add the following to `Cargo.toml` or `.cargo/config.toml`:
```rust
[profile.dev.package.rinja_derive]
opt-level = 3
```

This may affect clean compile times in debug mode, but incremental compiles
will be faster.
