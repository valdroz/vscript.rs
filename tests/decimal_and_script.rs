//! Tests mirroring Java `DecimalEvalTests.java` and `ScriptEvalTests.java`.

use vscript::configuration;
use vscript::{DefaultRunBlock, DefaultVariantContainer, EquationEval, Variant, VariantContainer};

static GLOBAL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn eval_with(c: &mut DefaultVariantContainer, source: &str) -> Variant {
    EquationEval::new(source).unwrap().eval(c).unwrap()
}

#[test]
fn test_division_and_scale() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    let restore = configuration::set_decimal_scale(4);
    let mut c = DefaultVariantContainer::new();
    c.set_variant("d1", Variant::from_i32(1));
    c.set_variant("d2", Variant::from_i32(3));
    c.set_variant("s1", Variant::from_str("10.1"));

    let v = eval_with(&mut c, "d1/d2");
    assert!(v.is_numeric());
    assert_eq!(v.as_string(), "0.3333");

    let v = eval_with(&mut c, "d1 + s1");
    assert_eq!(v.as_string(), "11.1000");

    configuration::set_decimal_scale(restore);
}

#[test]
fn test_division_by_zero() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("d1", Variant::from_f64(1000.0));
    c.set_variant("d2", Variant::from_f64(0.0));
    assert!(EquationEval::new("d1/d2").unwrap().eval(&mut c).is_err());
}

#[test]
fn test_casting() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("d1", Variant::from_i32(2));
    c.set_variant("s1", Variant::from_str("10.1"));
    c.set_variant("s2", Variant::from_str("a10.1"));

    assert_eq!(eval_with(&mut c, "d1 + s1").as_numeric().unwrap().to_string().parse::<f64>().unwrap(), 12.1);
    assert_eq!(eval_with(&mut c, "s1 + d1").as_numeric().unwrap().to_string().parse::<f64>().unwrap(), 12.1);
    assert_eq!(eval_with(&mut c, "s2 + d1").as_string(), "a10.12");
    assert_eq!(eval_with(&mut c, "s1 * d1").as_numeric().unwrap().to_string().parse::<f64>().unwrap(), 20.2);
    assert_eq!(eval_with(&mut c, "s1 / d1").as_numeric().unwrap().to_string().parse::<f64>().unwrap(), 5.05);
    assert!((eval_with(&mut c, "s1 - d1").as_numeric().unwrap().to_string().parse::<f64>().unwrap() - 8.1).abs() < 1e-9);
}

#[test]
fn test_error_on_subtract_string() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("d1", Variant::from_i32(2));
    c.set_variant("s2", Variant::from_str("a10.1"));
    assert!(EquationEval::new("s2 - d1").unwrap().eval(&mut c).is_err());
}

#[test]
fn test_error_on_multiply_string() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("d1", Variant::from_i32(2));
    c.set_variant("s2", Variant::from_str("a10.1"));
    assert!(EquationEval::new("s2 * d1").unwrap().eval(&mut c).is_err());
}

#[test]
fn test_error_on_divide_string() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("d1", Variant::from_i32(2));
    c.set_variant("s2", Variant::from_str("a10.1"));
    assert!(EquationEval::new("s2 / d1").unwrap().eval(&mut c).is_err());
    assert!(EquationEval::new("d1 / s2").unwrap().eval(&mut c).is_err());
}

// ----- ScriptEvalTests (user-defined functions) -----

#[test]
fn test_custom_function_evaluation() {
    let mut c = DefaultVariantContainer::new();
    let mut run_block = DefaultRunBlock::new();
    run_block
        .register_closure("multiply(first, second)", |lvc| {
            let first = lvc.get_variant("first");
            let second = lvc.get_variant("second");
            first.multiply(&second)
        })
        .unwrap();

    let result = EquationEval::new("2 + multiply(3, 4)")
        .unwrap()
        .eval_with(&run_block, &mut c)
        .unwrap();
    assert_eq!(result.as_numeric().unwrap().to_string().parse::<f64>().unwrap(), 14.0);
}

#[test]
fn test_custom_function_null_substitution() {
    let mut run_block = DefaultRunBlock::new();
    run_block
        .register_closure("produce_null(first, second)", |_lvc| Ok(Variant::null()))
        .unwrap();

    let result = EquationEval::new("2 + produce_null(1, 2)?-10")
        .unwrap()
        .eval_with(&run_block, &mut DefaultVariantContainer::new())
        .unwrap();
    assert_eq!(result.as_numeric().unwrap().to_string(), "-8");
}
