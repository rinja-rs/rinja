use rinja::Template;

#[derive(Template)]
#[template(source = "{{ b(value) }}", ext = "txt")]
struct OneFunction {
    value: u32,
}

impl OneFunction {
    fn b(&self, x: &u32) -> u32 {
        self.value + x
    }
}

#[test]
fn test_one_func() {
    let t = OneFunction { value: 10 };
    assert_eq!(t.render().unwrap(), "20");
}

#[derive(Template)]
#[template(source = "{{ self.func(value) }}", ext = "txt")]
struct OneFunctionSelf {
    value: i32,
}

impl OneFunctionSelf {
    fn func(&self, i: &i32) -> i32 {
        2 * i
    }
}

#[test]
fn test_one_func_self() {
    let t = OneFunctionSelf { value: 123 };
    assert_eq!(t.render().unwrap(), "246");
}

#[derive(Template)]
#[template(source = "{{ func[index](value) }}", ext = "txt")]
struct OneFunctionIndex<'a> {
    func: &'a [fn(&i32) -> i32],
    value: i32,
    index: usize,
}

#[test]
fn test_one_func_index() {
    let t = OneFunctionIndex {
        func: &[|_| panic!(), |&i| 2 * i, |_| panic!(), |_| panic!()],
        value: 123,
        index: 1,
    };
    assert_eq!(t.render().unwrap(), "246");
}

struct AddToGetAFunction;

impl std::ops::Add<usize> for &AddToGetAFunction {
    type Output = fn(&i32) -> i32;

    fn add(self, rhs: usize) -> Self::Output {
        assert_eq!(rhs, 1);
        |&i| 2 * i
    }
}

#[derive(Template)]
#[template(source = "{{ (func + index)(value) }}", ext = "txt")]
struct OneFunctionBinop<'a> {
    func: &'a AddToGetAFunction,
    value: i32,
    index: usize,
}

#[test]
fn test_one_func_binop() {
    let t = OneFunctionBinop {
        func: &AddToGetAFunction,
        value: 123,
        index: 1,
    };
    assert_eq!(t.render().unwrap(), "246");
}

fn double_attr_arg_helper(x: u32) -> u32 {
    x * x + x
}

#[derive(rinja::Template)]
#[template(
    source = "{{ self::double_attr_arg_helper(self.x.0 + 2) }}",
    ext = "txt"
)]
struct DoubleAttrArg {
    x: (u32,),
}

#[test]
fn test_double_attr_arg() {
    let t = DoubleAttrArg { x: (10,) };
    assert_eq!(t.render().unwrap(), "156");
}

// Ensures that fields are not moved when calling a jinja macro.
#[derive(Template)]
#[template(
    source = "
{%- macro package_navigation(title, show) -%}
{%- if show -%}
{{title}}
{%- else -%}
no show
{%- endif -%}
{%- endmacro -%}

{%- call package_navigation(title=title, show=true) -%}
",
    ext = "html"
)]
struct DoNotMoveFields {
    title: String,
}

#[test]
fn test_do_not_move_fields() {
    let x = DoNotMoveFields {
        title: "a".to_string(),
    };
    assert_eq!(x.render().unwrap(), "a");
}

#[derive(Template)]
#[template(source = "{{ (func)(value) }}", ext = "txt")]
struct ClosureField {
    func: fn(&i32) -> i32,
    value: i32,
}

#[test]
fn test_closure_field() {
    let t = ClosureField {
        func: |&i| 2 * i,
        value: 123,
    };
    assert_eq!(t.render().unwrap(), "246");
}

fn single() -> &'static str {
    "a"
}

mod sub_mod {
    pub fn sub_fn(v: i32) -> i32 {
        v * 2
    }
}

#[derive(Template)]
#[template(
    source = "
{{- self::single() -}}
{{- sub_mod::sub_fn(3) -}}
",
    ext = "txt"
)]
struct NotMethod;

#[test]
fn test_not_method() {
    assert_eq!(NotMethod.render().unwrap(), "a6");
}
