//! Tests mirroring the Java test-suite (`EquationEvalTests.java` and friends).

use vscript::configuration;
use vscript::{
    set_current_time_ms, DefaultRunBlock, DefaultVariantContainer,
    EquationEval, Variant, VariantContainer,
};

/// Global-state (time/config overrides) tests must not race. Rust runs tests
/// on multiple threads; hold this lock for any test that mutates globals.
static GLOBAL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn eval_str(source: &str) -> Variant {
    EquationEval::new(source)
        .unwrap()
        .eval(&mut DefaultVariantContainer::new())
        .unwrap()
}

fn num(v: &Variant) -> f64 {
    v.as_numeric().unwrap().to_string().parse().unwrap()
}

#[test]
fn test_equation_evaluator() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("movie.price", Variant::from_i32(5));
    let v = EquationEval::new("3 + movie.price * sqrt(4)")
        .unwrap()
        .eval(&mut c)
        .unwrap();
    assert_eq!(num(&v), 13.0);
}

#[test]
fn test_equation_evaluator_with_more() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("movie.price", Variant::from_i32(5));
    assert!(EquationEval::new("3 + movie.price * sqrt(4) >= 13.0").unwrap().eval(&mut c).unwrap().as_boolean());
    assert!(EquationEval::new("3 + movie.price * sqrt(4) == 13.0").unwrap().eval(&mut c).unwrap().as_boolean());
    assert!(EquationEval::new("3 + movie.price * sqrt(4) <= 13.0").unwrap().eval(&mut c).unwrap().as_boolean());
    assert!(!EquationEval::new("3 + movie.price * sqrt(4) < 13.0").unwrap().eval(&mut c).unwrap().as_boolean());
    assert!(!EquationEval::new("3 + movie.price * sqrt(4) >= 13.1").unwrap().eval(&mut c).unwrap().as_boolean());
}

#[test]
fn test_booleans_arithmetic_error() {
    assert!(EquationEval::new("true + true").unwrap().eval(&mut DefaultVariantContainer::new()).is_err());
}

#[test]
fn test_booleans_and() {
    let v = eval_str("true && true");
    assert!(v.is_boolean() && v.as_boolean());
}

#[test]
fn test_string_plus_numeric() {
    let v = eval_str("\"2\" + 1");
    assert!(v.is_numeric());
    assert_eq!(v.as_string(), "3");
}

#[test]
fn test_negative_and_priority() {
    assert_eq!(num(&eval_str("-1.0 * (2.5 + 3.5)")), -6.0);
}

#[test]
fn test_assignment() {
    let mut c = DefaultVariantContainer::new();
    EquationEval::new("a = -1.0 * (2.5 + 3.5)").unwrap().eval(&mut c).unwrap();
    assert_eq!(num(&c.get_variant("a")), -6.0);
}

#[test]
fn test_const_assignment_error() {
    assert!(EquationEval::new("true = 1").is_err());
}

#[test]
fn test_equal_priority_plus() {
    assert!(eval_str("1+2+3 == 6").as_boolean());
}

#[test]
fn test_equal_priority_minus() {
    assert!(eval_str("0 > 4 - 3 - 2").as_boolean());
}

#[test]
fn test_equal_priority_multi_div() {
    assert!(eval_str("2*3*4 == 24").as_boolean());
}

#[test]
fn test_priority_sum_mul() {
    assert_eq!(num(&eval_str("2+3*4")), 14.0);
}

#[test]
fn test_priority_mul_sum() {
    assert_eq!(num(&eval_str("3*4+2")), 14.0);
}

#[test]
fn test_boolean_error_on_arithmetics() {
    assert!(EquationEval::new("false + 1").unwrap().eval(&mut DefaultVariantContainer::new()).is_err());
}

#[test]
fn test_negated_priority() {
    let v = eval_str("!(false || true)");
    assert!(v.is_boolean() && !v.as_boolean());
}

#[test]
fn test_day_func() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391075000));
    assert_eq!(num(&eval_str("day()")), 5.0);
    set_current_time_ms(None);
}

#[test]
fn test_month_func() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391075000));
    assert_eq!(num(&eval_str("month()")), 2.0);
    set_current_time_ms(None);
}

#[test]
fn test_year_func() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391075000));
    assert_eq!(num(&eval_str("year()")), 2010.0);
    set_current_time_ms(None);
}

#[test]
fn test_days_in_month() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391075000));
    assert_eq!(num(&eval_str("days_in_month(2)")), 30.0);
    assert_eq!(num(&eval_str("days_in_month(-1)")), 31.0);
    assert_eq!(num(&eval_str("days_in_month()")), 28.0);
    set_current_time_ms(None);
}

#[test]
fn test_iso_func() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("testDate", Variant::from_str("2010-02-05T17:31:15Z"));
    let v = EquationEval::new("iso(testDate)").unwrap().eval(&mut c).unwrap();
    assert_eq!(v.as_numeric().unwrap().to_string(), "1265391075000");
}

#[test]
fn test_day_of_week() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("isoDate", Variant::from_str("2010-02-05T17:31:15Z"));
    c.set_variant("millisDate", Variant::from_i64(1265391075000));
    assert_eq!(num(&eval_str("day_of_week(\"2010-02-05T17:31:15Z\")")), 5.0);
    assert_eq!(num(&EquationEval::new("day_of_week(isoDate)").unwrap().eval(&mut c).unwrap()), 5.0);
    assert_eq!(num(&eval_str("day_of_week(1265391075000)")), 5.0);
    assert_eq!(num(&EquationEval::new("day_of_week(millisDate)").unwrap().eval(&mut c).unwrap()), 5.0);
    assert_eq!(num(&eval_str("day_of_week(\"2024-01-05T23:30:00-05:00\")")), 5.0);
    assert_eq!(num(&eval_str("day_of_week(\"2024-01-06T04:30:00Z\")")), 6.0);
}

#[test]
fn test_day_of_week_invalid() {
    assert!(EquationEval::new("day_of_week(false)").unwrap().eval(&mut DefaultVariantContainer::new()).is_err());
    assert!(EquationEval::new("day_of_week(\"not-a-date\")").unwrap().eval(&mut DefaultVariantContainer::new()).is_err());
}

#[test]
fn test_days_before_now() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391075000));
    let mut c = DefaultVariantContainer::new();
    c.set_variant("testDate", Variant::from_str("2010-02-01"));
    assert_eq!(num(&EquationEval::new("days_before_now(testDate)").unwrap().eval(&mut c).unwrap()), 4.0);
    c.set_variant("testDate", Variant::from_str("2009-02-01"));
    assert_eq!(num(&EquationEval::new("days_before_now(testDate)").unwrap().eval(&mut c).unwrap()), 369.0);
    assert_eq!(num(&EquationEval::new("days_before_now(now() - 86400000)").unwrap().eval(&mut c).unwrap()), 1.0);
    set_current_time_ms(None);
}

#[test]
fn test_hours_before_now() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391075000));
    let mut c = DefaultVariantContainer::new();
    c.set_variant("testDate", Variant::from_str("2010-02-05T10:00:00Z"));
    assert_eq!(num(&EquationEval::new("hours_before_now(testDate)").unwrap().eval(&mut c).unwrap()), 7.0);
    set_current_time_ms(None);
}

#[test]
fn test_minutes_before_now() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    set_current_time_ms(Some(1265391000000));
    let mut c = DefaultVariantContainer::new();
    c.set_variant("testDate", Variant::from_str("2010-02-05T17:00:00Z"));
    assert_eq!(num(&EquationEval::new("minutes_before_now(testDate)").unwrap().eval(&mut c).unwrap()), 30.0);
    set_current_time_ms(None);
}

#[test]
fn test_floor_mod() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("x", Variant::from_i32(15));
    c.set_variant("y", Variant::from_i32(10));
    assert_eq!(num(&EquationEval::new("floor_mod(x,y)").unwrap().eval(&mut c).unwrap()), 5.0);
    c.set_variant("a", Variant::from_i32(27));
    c.set_variant("b", Variant::from_i32(9));
    assert_eq!(num(&EquationEval::new("floor_mod(a,b)").unwrap().eval(&mut c).unwrap()), 0.0);
}

#[test]
fn test_substitutions() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("a", Variant::null());
    c.set_variant("b", Variant::from_i32(1));
    c.set_variant("c", Variant::from_str("c"));
    c.set_variant("a2", Variant::null());

    assert_eq!(num(&EquationEval::new("2 + a?b + 2").unwrap().eval(&mut c).unwrap()), 5.0);
    assert_eq!(num(&EquationEval::new("a?a2?b + 2").unwrap().eval(&mut c).unwrap()), 3.0);
    assert_eq!(num(&EquationEval::new("a?sqrt(4) + 1").unwrap().eval(&mut c).unwrap()), 3.0);
    assert!(EquationEval::new("a?true").unwrap().eval(&mut c).unwrap().as_boolean());
    assert!(!EquationEval::new("a?false").unwrap().eval(&mut c).unwrap().as_boolean());
    assert_eq!(EquationEval::new("a?\"aaa\" + 2").unwrap().eval(&mut c).unwrap().as_string(), "aaa2");
    assert_eq!(num(&EquationEval::new("a?-1 + 2").unwrap().eval(&mut c).unwrap()), 1.0);
}

#[test]
fn test_leading_negative_digit() {
    assert_eq!(num(&eval_str("-123 + 2 * 3")), -117.0);
    assert_eq!(num(&eval_str("(-123 + 2) * 3")), -363.0);
}

#[test]
fn test_empty_and_default_expression() {
    let _g = GLOBAL_LOCK.lock().unwrap();
    assert!(eval_str("   ").is_null());
    let backup = configuration::set_expression_for_empty_eval(Some("true".into()));
    let v = eval_str("   ");
    configuration::set_expression_for_empty_eval(Some(backup));
    assert!(v.is_boolean() && v.as_boolean());
}

#[test]
fn test_invalid_assignments() {
    assert!(EquationEval::new("1=3").is_err());
}

#[test]
fn test_stats() {
    let eq = EquationEval::new("c[0] = a + b; d = extf1(c[0] + e + 1); d = sin(a * 0); c[f] = extf2() ").unwrap();
    let stats = eq.stats();
    let mut vars: Vec<_> = stats.referenced_variables().cloned().collect();
    vars.sort();
    assert_eq!(vars, vec!["a", "b", "c", "d", "e", "f"]);
    let mut fns: Vec<_> = stats.referenced_ext_functions().cloned().collect();
    fns.sort();
    assert_eq!(fns, vec!["extf1", "extf2"]);
}

#[test]
fn test_custom_function_via_run_block() {
    let mut c = DefaultVariantContainer::new();
    let mut master = DefaultRunBlock::new();
    master.register_closure("multiply10(a)", |c| {
        let a = c.get_variant("a");
        Ok(Variant::from_f64(
            a.as_numeric().unwrap().to_string().parse::<f64>().unwrap() * 10.0,
        ))
    })
    .unwrap();
    let eval = EquationEval::new("multiply10(3)").unwrap();
    let v = eval.eval_with(&master, &mut c).unwrap();
    assert_eq!(num(&v), 30.0);
}

#[test]
fn test_power() {
    assert_eq!(num(&eval_str("pow(1+1,\"3\")")), 8.0);
    assert_eq!(num(&eval_str("pow(\"2\",\"3\" + 1)")), 16.0);
    // With a trace listener attached, errors on a node are caught and the node
    // yields null (mirroring Java TracingBaseNode). Untraced, this would error.
    let eval = EquationEval::new("pow(\"a\",\"3\" + 1)").unwrap();
    let mut c = DefaultVariantContainer::new();
    let v = eval
        .eval_traced(&vscript::NoopRunBlock, &mut c, &|_msg: &str| {})
        .unwrap();
    assert!(v.is_null());
}

#[test]
fn test_cosine() {
    assert_eq!(num(&eval_str("cos(0)")), 1.0);
    assert!(eval_str("cos(null)").is_null());
    assert!(EquationEval::new("cos(false)").unwrap().eval(&mut DefaultVariantContainer::new()).is_err());
    assert!(eval_str("a=cos(to_array(0,0,PI)); a[0]==1 && a[1]==1 && a[2]==-1").as_boolean());
}

#[test]
fn test_bin_operators() {
    assert!(eval_str("1|2 == 3 && 1&2 == 0 && 1^3==2").as_boolean());
}

#[test]
fn test_not_on_null() {
    assert!(eval_str("a=null; !a").is_null());
}

#[test]
fn test_not_on_number() {
    let v = eval_str("a=24; !a");
    assert!(v.is_boolean() && !v.as_boolean());
}

#[test]
fn test_not_on_zero() {
    let v = eval_str("a=0; !a");
    assert!(v.is_boolean() && v.as_boolean());
}

#[test]
fn test_not_on_string_true() {
    let v = eval_str("a=\"true\"; !a");
    assert!(v.is_boolean() && !v.as_boolean());
}

#[test]
fn test_if_operation() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("a", Variant::from_i32(10));
    c.set_variant("b", Variant::from_i32(20));
    assert_eq!(num(&EquationEval::new("if(a < b , a, b)").unwrap().eval(&mut c).unwrap()), 10.0);
    c.set_variant("c", Variant::from_str("truestatement"));
    c.set_variant("d", Variant::from_str("falsestatement"));
    assert_eq!(
        EquationEval::new("if(a < b , c, d , e)").unwrap().eval(&mut c).unwrap().as_string(),
        "truestatement"
    );
}

#[test]
fn test_function_first() {
    assert_eq!(eval_str("first(\"Hello\", 2)").as_string(), "He");
    assert_eq!(eval_str("first(\"Hello\", 10)").as_string(), "Hello");
    assert_eq!(eval_str("first(\"Hello\", 0)").as_string(), "");
    assert_eq!(eval_str("first(\"Hello\", -2)").as_string(), "");
}

#[test]
fn test_function_last() {
    assert_eq!(eval_str("last(\"Hello\", 2)").as_string(), "lo");
    assert_eq!(eval_str("last(\"Hello\", 10)").as_string(), "Hello");
    assert_eq!(eval_str("last(\"Hello\", 0)").as_string(), "");
    assert_eq!(eval_str("last(\"Hello\", -2)").as_string(), "");
}

#[test]
fn test_function_skip() {
    assert_eq!(eval_str("skip(\"Hello\", 2)").as_string(), "llo");
    assert_eq!(eval_str("skip(\"Hello\", 10)").as_string(), "");
    assert_eq!(eval_str("skip(\"Hello\", 0)").as_string(), "Hello");
    assert_eq!(eval_str("skip(\"Hello\", -2)").as_string(), "Hello");
}

#[test]
fn test_function_avg() {
    assert_eq!(num(&eval_str("avg(1,2,3,4,5)")), 3.0);
    assert!((num(&eval_str("avg(1.1,2.1,3.1,4.1,5.1)")) - 3.1).abs() < 1e-9);
    assert!(eval_str("avg()").is_null());
}

#[test]
fn test_function_median() {
    assert_eq!(num(&eval_str("median(1, 3, 3, 6, 7, 8, 9)")), 6.0);
    assert_eq!(num(&eval_str("median(1, 2, 3, 4, 5, 6, 8, 9)")), 4.5);
    assert_eq!(num(&eval_str("median(7, 8, 9)")), 8.0);
    assert_eq!(num(&eval_str("median(1, 3)")), 2.0);
    assert_eq!(num(&eval_str("median(3)")), 3.0);
    assert!(eval_str("median()").is_null());
}

#[test]
fn test_function_max() {
    assert_eq!(num(&eval_str("max(3, -100, 234, 234.1, null, 7, 8, 9)")), 234.1);
    assert_eq!(num(&eval_str("max(3, -100, 234, \"234.1\", null, 7, 8, 9)")), 234.1);
    assert_eq!(num(&eval_str("max(3, to_array(-100, 234, 234.1, null), 7, 8, 9)")), 234.1);
    assert!(eval_str("max(\"a\",\"x\",\"c\")").is_null());
    assert_eq!(num(&eval_str("max(5, {10, 11}, 3)")), 11.0);
}

#[test]
fn test_function_min() {
    assert_eq!(num(&eval_str("min(null, 3, -100, 234, 234.1, 7, null, 8, 9)")), -100.0);
    assert_eq!(num(&eval_str("min(3, to_array(-100, 234, 234.1, null), 7, null, 8, 9)")), -100.0);
    assert_eq!(num(&eval_str("min(5, {10, 2}, 3)")), 2.0);
    assert_eq!(num(&eval_str("min(5, {10, 15}, 3)")), 3.0);
}

#[test]
fn test_switch_operation() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("switch_on", Variant::from_i32(1));
    assert_eq!(num(&EquationEval::new("switch(switch_on, 1, switch_on * 2, 2, switch_on * 3)").unwrap().eval(&mut c).unwrap()), 2.0);

    let mut c2 = DefaultVariantContainer::new();
    c2.set_variant("switch_on", Variant::from_i32(2));
    assert_eq!(num(&EquationEval::new("switch(switch_on, 1, switch_on * 2, 2, switch_on * 3)").unwrap().eval(&mut c2).unwrap()), 6.0);

    let mut c3 = DefaultVariantContainer::new();
    c3.set_variant("a", Variant::from_i32(20));
    c3.set_variant("b", Variant::from_i32(2));
    assert_eq!(num(&EquationEval::new("switch( a * b , 10, 10, 20, 20, 40)").unwrap().eval(&mut c3).unwrap()), 40.0);
}

#[test]
fn test_round() {
    assert_eq!(num(&eval_str("round(10.1, 0)")), 10.0);
    assert_eq!(num(&eval_str("round(10.9, 0)")), 11.0);
    assert_eq!(num(&eval_str("round(10.5, 0)")), 11.0);
    assert_eq!(num(&eval_str("round(10.555, 2)")), 10.56);
    assert_eq!(num(&eval_str("round(11/3, 0)")), 4.0);
    assert_eq!(num(&eval_str("round(11/3, 2)")), 3.67);

    let mut c = DefaultVariantContainer::new();
    c.set_variant("fact", Variant::from_f64(7.2));
    assert_eq!(num(&EquationEval::new("round(fact/2, 1)").unwrap().eval(&mut c).unwrap()), 3.6);
    assert_eq!(num(&eval_str("round(-10.5, 0)")), -11.0);
    assert!(EquationEval::new("round(1, -1)").unwrap().eval(&mut DefaultVariantContainer::new()).is_err());
}

#[test]
fn test_format_ts() {
    let mut c = DefaultVariantContainer::new();
    assert_eq!(
        EquationEval::new("format_ts(1683900000000, \"%m/%d/%Y\")").unwrap().eval(&mut c).unwrap().as_string(),
        "05/12/2023"
    );
}

#[test]
fn test_undefined_function_error() {
    let mut c = DefaultVariantContainer::new();
    assert!(EquationEval::new("nosuchfunc(1)").unwrap().eval(&mut c).is_err());
}

#[test]
fn test_arrays() {
    // Array creation with { }
    let v = eval_str("a = {1,2,3}; size(a)");
    assert_eq!(num(&v), 3.0);
    // to_array function
    let v = eval_str("size(to_array(10, 20))");
    assert_eq!(num(&v), 2.0);
    // Indexing
    let v = eval_str("a = {1,2,3}; a[1]");
    assert_eq!(num(&v), 2.0);
    // Set index
    let v = eval_str("a = {1,2,3}; a[0] = 10; a[0]");
    assert_eq!(num(&v), 10.0);
    // is_array
    assert!(eval_str("is_array({1,2})").as_boolean());
    assert!(!eval_str("is_array(5)").as_boolean());
}
