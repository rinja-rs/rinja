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
impl<'a> askama::Template for HelloWorld<'a> {
    fn render_into<AskamaW>(&self, __askama_writer: &mut AskamaW) -> askama::Result<()>
    where
        AskamaW: core::fmt::Write + ?Sized,
    {
        __askama_writer.write_str("Hello, ")?;
        match (
            &((&&askama::filters::AutoEscaper::new(
                &(self.name),
                askama::filters::Html,
            ))
                .askama_auto_escape()?),
        ) {
            (expr2,) => {
                (&&askama::filters::Writable(expr2)).askama_write(__askama_writer)?;
            }
        }
        __askama_writer.write_str("!")?;
        Ok(())
    }
    const SIZE_HINT: usize = 11usize;
}
```
