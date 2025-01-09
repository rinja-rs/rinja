# Runtime values

It is possible to define variables at runtime and to use them in the templates using the `VALUES`
variable. To do so you need to use the `_with_values` variants of the `render` methods. It expects
an extra argument of type `Values`:

```rust
use rinja::Values;

let mut values = Values::new();
// We add a new value named "name" with the value "Bibop".
values.add("name", "Bibop");
// We add a new value named "another" with a vec.
values.add("another", vec![false]);
```

The `Values` type is storing data with the `Any` trait, allowing to store any type as long as it
implements this trait.

Then to render with these values:

```rust
TemplateStruct.render_with_values(&values).unwrap();
```

And to use them in a template:

```jinja
{% if let Ok(name) = VALUES.get::<&str>() %}
  name is {{ name }}
{% endif %}
```

If you try to retrieve a value with the wrong type or that you didn't set, you will get an
`Err(ValueError)`.
