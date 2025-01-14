# Debugging and Troubleshooting

You can view the parse tree for a template as well as the generated code by
changing the `template` attribute item list for the template struct:

```rust
#[derive(Template)]
#[template(path = "hello.html", print = "all")]
struct HelloTemplate<'a> { ... }
```

The `print` key can take one of four values:

* `none` (the default value)
* `ast` (print the parse tree)
* `code` (print the generated code)
* `all` (print both parse tree and code)

The resulting output will be printed to `stderr` during the compilation process.

The parse tree looks like this for the example template:

```rust
[Lit("", "Hello,", " "), Expr(WS(false, false), Var("name")), Lit("", "!", "\n")]
```

The generated code looks like this:

```rust
impl<'a> rinja::Template for HelloWorld<'a> {
    fn render_into<RinjaW>(&self, __rinja_writer: &mut RinjaW) -> rinja::Result<()>
    where
        RinjaW: core::fmt::Write + ?Sized,
    {
        __rinja_writer.write_str("Hello, ")?;
        match (
            &((&&rinja::filters::AutoEscaper::new(
                &(self.name),
                rinja::filters::Html,
            ))
                .rinja_auto_escape()?),
        ) {
            (expr2,) => {
                (&&rinja::filters::Writable(expr2)).rinja_write(__rinja_writer)?;
            }
        }
        __rinja_writer.write_str("!")?;
        Ok(())
    }
    const SIZE_HINT: usize = 11usize;
}
```
