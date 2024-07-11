use rinja::Template;

#[test]
fn test_abcd() {
    #[derive(Template)]
    #[template(
        source = "
        {%- ifdef a -%} a={{a}} {%- endifdef -%}
        {%- ifdef any(b, c) -%}
            {%- ifdef b -%} b={{b}} {%- endifdef -%}
            {%- ifdef c -%} c={{c}} {%- endifdef -%}
        {%- endifdef -%}
        {%- ifdef not(d) -%} d=! {%- endifdef -%}
        {%- ifdef all(a, b, c) -%} 4 {%- endifdef -%}
    ",
        ext = "txt"
    )]
    #[allow(dead_code)]
    struct Abcd {
        a: u32,
        b: u32,
        c: u32,
        d: u32,
    }

    assert_eq!(
        Abcd {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        }
        .to_string(),
        "a=1b=2c=34"
    );
}

#[test]
fn test_abc() {
    #[derive(Template)]
    #[template(
        source = "
        {%- ifdef a -%} a={{a}} {%- endifdef -%}
        {%- ifdef any(b, c) -%}
            {%- ifdef b -%} b={{b}} {%- endifdef -%}
            {%- ifdef c -%} c={{c}} {%- endifdef -%}
        {%- endifdef -%}
        {%- ifdef not(d) -%} d=! {%- endifdef -%}
        {%- ifdef all(a, b, c) -%} 4 {%- endifdef -%}
    ",
        ext = "txt"
    )]
    #[allow(dead_code)]
    struct Abc {
        a: u32,
        b: u32,
        c: u32,
    }

    assert_eq!(Abc { a: 1, b: 2, c: 3 }.to_string(), "a=1b=2c=3d=!4");
}

#[test]
fn test_ab() {
    #[derive(Template)]
    #[template(
        source = "
        {%- ifdef a -%} a={{a}} {%- endifdef -%}
        {%- ifdef any(b, c) -%}
            {%- ifdef b -%} b={{b}} {%- endifdef -%}
            {%- ifdef c -%} c={{c}} {%- endifdef -%}
        {%- endifdef -%}
        {%- ifdef not(d) -%} d=! {%- endifdef -%}
        {%- ifdef all(a, b, c) -%} 4 {%- endifdef -%}
    ",
        ext = "txt"
    )]
    #[allow(dead_code)]
    struct Ab {
        a: u32,
        b: u32,
    }

    assert_eq!(Ab { a: 1, b: 2 }.to_string(), "a=1b=2d=!");
}

#[test]
fn test_b() {
    #[derive(Template)]
    #[template(
        source = "
        {%- ifdef a -%} a={{a}} {%- endifdef -%}
        {%- ifdef any(b, c) -%}
            {%- ifdef b -%} b={{b}} {%- endifdef -%}
            {%- ifdef c -%} c={{c}} {%- endifdef -%}
        {%- endifdef -%}
        {%- ifdef not(d) -%} d=! {%- endifdef -%}
        {%- ifdef all(a, b, c) -%} 4 {%- endifdef -%}
    ",
        ext = "txt"
    )]
    #[allow(dead_code)]
    struct B {
        b: u32,
    }

    assert_eq!(B { b: 2 }.to_string(), "b=2d=!");
}
