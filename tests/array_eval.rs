//! Tests mirroring Java `ArrayEvalTests.java`.

use vscript::{DefaultVariantContainer, EquationEval, Variant, VariantContainer};

fn eval_with(c: &mut DefaultVariantContainer, source: &str) -> Variant {
    EquationEval::new(source).unwrap().eval(c).unwrap()
}

#[test]
fn test_add_to_array() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("arr", Variant::empty_array());

    let v = eval_with(&mut c, "arr = arr + \"a\" + 2 + 3");
    let arr = v.as_array();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_string(), "a");

    let v = eval_with(&mut c, "arr[0]=1");
    let arr = v.as_array();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_numeric().unwrap().to_string(), "1");
}

#[test]
fn test_array_value_substitution() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("arr", Variant::from_array(vec![Variant::from_str("a")]));
    let v = eval_with(&mut c, "arr[0] + arr[1]?\"b\"");
    assert_eq!(v.as_string(), "ab");
}

#[test]
fn test_array_index_value_replacement() {
    let mut c = DefaultVariantContainer::new();
    c.set_variant("arr", Variant::from_array(vec![Variant::from_str("a")]));
    let v = eval_with(&mut c, "arr[0]=2; arr[1]=arr[0]; arr[0]==arr[1]");
    assert!(v.as_boolean());
    let arr = c.get_variant("arr");
    assert_eq!(arr.as_array().len(), 2);
    assert_eq!(arr.as_array()[0].as_numeric().unwrap().to_string(), "2");
}

#[test]
fn test_to_array_function() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(&mut c, "arr = to_array(1,2,\"3\",true); is_size_4 = size(arr); arr == 3");
    assert!(v.as_boolean());
    let arr = c.get_variant("arr");
    assert_eq!(arr.as_array().len(), 4);
    // string "3" == numeric 3 matches
    assert!(arr.as_array()[2].is_string());
}

#[test]
fn test_array_concatenation() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(
        &mut c,
        "a = to_array(1,2,3,4) + to_array(5,6,7,8); size(a) == 8 && a[0]==1 && a[7]==8",
    );
    assert!(v.as_boolean());
    assert_eq!(c.get_variant("a").as_array().len(), 8);
}

#[test]
fn test_array_subtraction() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(
        &mut c,
        "a = to_array(1,2,4,3,4) - to_array(1,4,8); size(a) == 2 && a[0]==2 && a[1]==3",
    );
    assert!(v.as_boolean());
}

#[test]
fn test_remove_matching_items() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(&mut c, "a=to_array(1,2,4,3,4); a - 4");
    let arr = v.as_array();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_numeric().unwrap().to_string(), "1");
    assert_eq!(arr[1].as_numeric().unwrap().to_string(), "2");
    assert_eq!(arr[2].as_numeric().unwrap().to_string(), "3");
}

#[test]
fn test_numeric_minus_array_uses_size() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(&mut c, "a=to_array(1,2,4,3,4); 4 - a");
    assert_eq!(v.as_numeric().unwrap().to_string(), "-1");
}

#[test]
fn test_multiply_not_supported_for_arrays() {
    let mut c = DefaultVariantContainer::new();
    assert!(EquationEval::new("a=to_array(1,2,4,3,4); a * 4")
        .unwrap()
        .eval(&mut c)
        .is_err());
}

#[test]
fn test_dynamic_array_allocation() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(
        &mut c,
        "a[2]=10; size(a) == 3; a[0] == null && a[2] == 10",
    );
    assert!(v.as_boolean());
}

#[test]
fn test_dynamic_array_reallocation() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(
        &mut c,
        "a=1; a[1]=2; a[2]=3; size(a) == 3 && a[0]==1 && a[1]==2 && a[2]==3",
    );
    assert!(v.as_boolean());
}

#[test]
fn test_braced_array_assignment() {
    let mut c = DefaultVariantContainer::new();
    let v = eval_with(
        &mut c,
        "arr = {1,2,\"3\",true} + \"added to the end\"; is_size_4 = size(arr); arr == 3",
    );
    assert!(v.as_boolean());
    let arr = c.get_variant("arr");
    let items = arr.as_array();
    assert_eq!(items.len(), 5);
    assert_eq!(items[4].as_string(), "added to the end");
}
