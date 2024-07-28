# Rinja

Rinja implements a template rendering engine based on [Jinja](https://jinja.palletsprojects.com/).
It generates Rust code from your templates at compile time
based on a user-defined `struct` to hold the template's context.
See below for an example.

All feedback welcome. Feel free to file bugs, requests for documentation and
any other feedback to the [issue tracker][issues].

### Feature highlights

* Construct templates using a familiar, easy-to-use syntax
* Benefit from the safety provided by Rust's type system
* Template code is compiled into your crate for optimal performance
* Optional built-in support for Actix, Axum, Rocket, and warp web frameworks
* Debugging features to assist you in template development
* Templates must be valid UTF-8 and produce UTF-8 when rendered
* Works on stable Rust

### Supported in templates

* Template inheritance
* Loops, if/else statements and include support
* Macro support
* Variables (no mutability allowed)
* Some built-in filters, and the ability to use your own
* Whitespace suppressing with '-' markers
* Opt-out HTML escaping
* Syntax customization

[issues]: https://github.com/rinja-rs/rinja/issues
