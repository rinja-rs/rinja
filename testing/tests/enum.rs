use std::any::type_name_of_val;
use std::fmt::{Debug, Display};

use rinja::Template;

#[test]
fn test_simple_enum() {
    #[derive(Template, Debug)]
    #[template(
        ext = "txt",
        source = "{{ self::type_name_of_val(self) }} | {{ self|fmt(\"{:?}\") }}"
    )]
    enum SimpleEnum<'a, B: Display + Debug> {
        #[template(source = "A")]
        A,
        #[template(source = "B()")]
        B(),
        #[template(source = "C({{self.0}}, {{self.1}})")]
        C(u32, u32),
        #[template(source = "D {}")]
        D {},
        #[template(source = "E { a: {{a}}, b: {{b}} }")]
        E {
            a: &'a str,
            b: B,
        },
        // uses default source with `SimpleEnum` as `Self`
        F,
        // uses default source with a synthetic type `__Rinja__SimpleEnum__G` as `Self`
        #[template()]
        G,
    }

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::A;
    assert_eq!(tmpl.render().unwrap(), "A");

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::B();
    assert_eq!(tmpl.render().unwrap(), "B()");

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::C(12, 34);
    assert_eq!(tmpl.render().unwrap(), "C(12, 34)");

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::C(12, 34);
    assert_eq!(tmpl.render().unwrap(), "C(12, 34)");

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::D {};
    assert_eq!(tmpl.render().unwrap(), "D {}");

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::E { a: "hello", b: X };
    assert_eq!(tmpl.render().unwrap(), "E { a: hello, b: X }");

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::F;
    assert_eq!(
        tmpl.render().unwrap(),
        "&enum::test_simple_enum::SimpleEnum<enum::X> | F",
    );

    let tmpl: SimpleEnum<'_, X> = SimpleEnum::G;
    assert_eq!(
        tmpl.render().unwrap(),
        "&enum::test_simple_enum::_::__Rinja__SimpleEnum__G<enum::X> | \
        __Rinja__SimpleEnum__G(\
            PhantomData<&enum::test_simple_enum::SimpleEnum<enum::X>>\
        )",
    );
}

#[test]
fn test_enum_blocks() {
    #[derive(Template, Debug)]
    #[template(
        ext = "txt",
        source = "\
            {% block a -%} <a = {{ a }}> {%- endblock %}
            {% block b -%} <b = {{ b }}> {%- endblock %}
            {% block c -%} <c = {{ c }}> {%- endblock %}
            {% block d -%} <d = {{ self::type_name_of_val(self) }}> {%- endblock %}
        "
    )]
    enum BlockEnum<'a, C: Display> {
        #[template(block = "a")]
        A { a: u32 },
        #[template(block = "b")]
        B { b: &'a str },
        #[template(block = "c")]
        C { c: C },
        #[template(block = "d")]
        D,
    }

    let tmpl: BlockEnum<'_, X> = BlockEnum::A { a: 42 };
    assert_eq!(tmpl.render().unwrap(), "<a = 42>");

    let tmpl: BlockEnum<'_, X> = BlockEnum::B { b: "second letter" };
    assert_eq!(tmpl.render().unwrap(), "<b = second letter>");

    let tmpl: BlockEnum<'_, X> = BlockEnum::C { c: X };
    assert_eq!(tmpl.render().unwrap(), "<c = X>");

    assert_eq!(
        BlockEnum::<'_, X>::D.render().unwrap(),
        "<d = &enum::test_enum_blocks::_::__Rinja__BlockEnum__D<enum::X>>"
    );
}

#[derive(Debug)]
struct X;

impl Display for X {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("X")
    }
}
