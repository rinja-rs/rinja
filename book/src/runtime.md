# Runtime values

It is possible to define variables at runtime and to use them in the templates using the `VALUES`
variable. To do so you need to use the `_with_values` variants of the `render` methods. It expects
an extra argument implementing the `Values` trait. This trait is implemented on a few types provided
by the `std`, like `HashMap`:

```rust
use std::collections::HashMap;

let mut values: HashMap<String, Box<dyn Any>> = HashMap::new();
// We add a new value named "name" with the value "Bibop".
values.insert("name".into(), Box::new("Bibop"));
values.insert("age".into(), Box::new(12u32));
```

You can also use the methods provided by the `Values` trait to make it easier:

```rust
use rinja::Values;
use std::collections::HashMap;

let mut values: HashMap<String, Box<dyn Any>> = HashMap::new();
values.add_values("another", vec![false]);
values.add_values("bool", false);
```

The `Values` trait is expecting types storing data with the `Any` trait, allowing to store any type.

Then to render with these values:

```rust
TemplateStruct.render_with_values(&values).unwrap();
```

And to use them in a template:

```jinja
{% if let Ok(name) = VALUES.get_values::<&str>("name") %}
  name is {{ name }}
{% endif %}
{% if let Ok(age) = VALUES.get_values::<u32>("age") %}
  age is {{ age }}
{% endif %}
```

If you try to retrieve a value with the wrong type or that you didn't set, you will get an
`Err(ValueError)`.
