use vscript::{EquationEval, DefaultVariantContainer, Variant, VariantContainer};

#[test]
fn readme_quickstart() {
    let mut c = DefaultVariantContainer::new();
    let r = EquationEval::parse("3 + 5 * 2").unwrap().eval(&mut c).unwrap();
    assert_eq!(r.as_numeric().unwrap().to_string(), "13");

    let mut c = DefaultVariantContainer::new();
    let r = EquationEval::parse("a = 10; b = 20; c = a + b; c > 25").unwrap().eval(&mut c).unwrap();
    assert!(r.as_boolean());
}

#[test]
fn readme_variables() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("price", Variant::from_decimal_str("5.2").unwrap());
    let r = EquationEval::parse("price * 1.1").unwrap().eval(&mut c).unwrap();
    let v = r.as_numeric().unwrap();
    assert_eq!(v.round_dp(2).to_string(), "5.72");
}

#[test]
fn readme_custom() {
    use vscript::{DefaultRunBlock};
    let mut block = DefaultRunBlock::new();
    block
        .register_closure("custom_multiply(first, second)", |lvc| {
            let a = lvc.get_variant("first");
            let b = lvc.get_variant("second");
            a.multiply(&b)
        })
        .unwrap();
    let mut c = DefaultVariantContainer::new();
    let r = EquationEval::parse("2 + custom_multiply(3, 4)")
        .unwrap()
        .eval_with(&block, &mut c)
        .unwrap();
    assert_eq!(r.as_numeric().unwrap().to_string(), "14");
}

#[test]
fn readme_config() {
    use vscript::configuration;
    let restore = configuration::set_decimal_scale(2);
    let mut c = DefaultVariantContainer::new();
    let r = EquationEval::parse("res = 3 + 10.1; res == 13.1").unwrap().eval(&mut c).unwrap();
    assert!(r.as_boolean());
    configuration::set_decimal_scale(restore);
}
