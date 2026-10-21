//! Tests mirroring the Java `org.valdroz.vscript.json` test suite.

use vscript::container::VariantContainer;
use vscript::json::{
    variant_from_json, variant_to_json, DataSetIndex, DataSetMakerMode, JsonDataSetMaker,
    JsonVariantContainer,
};
use vscript::{EquationEval, Variant};

fn read_resource(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/resources/{}", env!("CARGO_MANIFEST_DIR"), name);
    let content = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

#[test]
fn test_json_variant_containers() {
    let json = read_resource("test-data-set.json");
    let containers = JsonVariantContainer::json_to_variant_containers(
        &json,
        &[DataSetMakerMode::KeepArraysForPrimitives],
    );

    assert_eq!(containers.len(), 2);
    let vc = &containers[0];
    assert_eq!(vc.get_variant("objects.type").as_string(), "obj1");
}

#[test]
fn test_json_eval_against_containers() {
    let json = read_resource("test-data-set.json");
    let containers = JsonVariantContainer::json_to_variant_containers(
        &json,
        &[DataSetMakerMode::KeepArraysForPrimitives],
    );

    let mut vc = containers[0].clone();
    let v = EquationEval::new(
        "size(options) == 3 && options == \"op2\" && objects.type == \"obj1\" && objects.p == 3 && objects.b",
    )
    .unwrap()
    .eval(&mut vc)
    .unwrap();
    assert!(v.as_boolean(), "first container eval failed");

    let mut vc = containers[1].clone();
    let v = EquationEval::new(
        "size(options) == 3 && options == \"op2\" && objects.type == \"obj2\" && objects.data == 0 && !is_null(objects.b) && !objects.b",
    )
    .unwrap()
    .eval(&mut vc)
    .unwrap();
    assert!(v.as_boolean(), "second container eval failed");
}

#[test]
fn test_leaf_array_interpretation() {
    let json = read_resource("test-data-array-interpretation.json");
    let containers = JsonVariantContainer::json_to_variant_containers(
        &json,
        &[DataSetMakerMode::KeepArraysForPrimitives],
    );
    assert_eq!(containers.len(), 1);
    let vc = &mut containers[0].clone();

    assert!(EquationEval::new("is_array(one) && size(one) == 1")
        .unwrap()
        .eval(vc)
        .unwrap()
        .as_boolean());
    assert!(EquationEval::new("is_array(two) && size(two) == 2")
        .unwrap()
        .eval(vc)
        .unwrap()
        .as_boolean());
    assert!(EquationEval::new("!is_array(single) && size(single) == 7")
        .unwrap()
        .eval(vc)
        .unwrap()
        .as_boolean());
}

#[test]
fn test_dataset_maker_keep_primitive_arrays() {
    let json = read_resource("test-data-set.json");
    let dm = JsonDataSetMaker::new(&json, "pref", &[DataSetMakerMode::KeepArraysForPrimitives]);
    let data_sets = dm.get_data_sets();
    assert_eq!(data_sets.len(), 2);
    assert_eq!(
        data_sets[0].get("pref.objects.type").unwrap().as_str().unwrap(),
        "obj1"
    );
    assert_eq!(
        data_sets[1].get("pref.objects.type").unwrap().as_str().unwrap(),
        "obj2"
    );
}

#[test]
fn test_dataset_maker_all_flattened_count() {
    let json = read_resource("test-data-set.json");
    let dm = JsonDataSetMaker::new(&json, "", &[DataSetMakerMode::AllFlattened]);
    assert_eq!(dm.get_data_sets().len(), 24);
}

#[test]
fn test_dataset_maker_2() {
    let json = read_resource("test-data-set-2.json");
    let dm = JsonDataSetMaker::new(&json, "", &[DataSetMakerMode::KeepArraysForPrimitives]);
    let data_sets = dm.get_data_sets();
    assert_eq!(data_sets.len(), 4);

    let expected = read_resource("test-data-set-2-result.json");
    let expected_arr = expected.as_array().unwrap().clone();
    for ds in &data_sets {
        assert!(
            expected_arr.contains(ds),
            "unexpected data set: {:?} not in {:?}",
            ds,
            expected_arr
        );
    }
}

#[test]
fn test_dataset_maker_3() {
    let json = read_resource("test-data-set-3.json");
    let dm = JsonDataSetMaker::new(&json, "", &[DataSetMakerMode::KeepArraysForPrimitives]);
    let data_sets = dm.get_data_sets();
    assert_eq!(data_sets.len(), 6);

    let expected = read_resource("test-data-set-3-result.json");
    let expected_arr = expected.as_array().unwrap().clone();
    for ds in &data_sets {
        assert!(
            expected_arr.contains(ds),
            "unexpected data set: {:?}",
            ds
        );
    }
}

#[test]
fn test_dataset_maker_4a() {
    let json = read_resource("test-data-set-4.json");
    let dm = JsonDataSetMaker::new(&json, "", &[DataSetMakerMode::KeepArraysForPrimitives]);
    let data_sets = dm.get_data_sets();
    assert_eq!(data_sets.len(), 24);

    let expected = read_resource("test-data-set-4a-result.json");
    let expected_arr = expected.as_array().unwrap().clone();
    for ds in &data_sets {
        assert!(expected_arr.contains(ds), "unexpected data set: {:?}", ds);
    }
}

#[test]
fn test_dataset_maker_4b() {
    let json = read_resource("test-data-set-4.json");
    let dm = JsonDataSetMaker::new(
        &json,
        "",
        &[
            DataSetMakerMode::KeepArraysForPrimitives,
            DataSetMakerMode::KeepComplexArrays,
        ],
    );
    let data_sets = dm.get_data_sets();
    assert_eq!(data_sets.len(), 24);

    let expected = read_resource("test-data-set-4b-result.json");
    let expected_arr = expected.as_array().unwrap().clone();
    for ds in &data_sets {
        assert!(expected_arr.contains(ds), "unexpected data set: {:?}", ds);
    }
}

#[test]
fn test_dataset_index_multiple_array_levels() {
    let json = read_resource("test-data-set-3.json");
    let dsi = DataSetIndex::index(&json);
    assert_eq!(dsi.number_of_permutations(), 6);

    for i in 0..6 {
        assert_eq!(dsi.get_value("id", i).as_string(), "1234567890");
        assert_eq!(
            dsi.get_value("prefs", i).as_array().len(),
            2,
            "prefs perm {}",
            i
        );
    }

    let expected_object_types = ["obj1", "obj1", "obj1", "obj2", "obj2", "obj2"];
    for (i, want) in expected_object_types.iter().enumerate() {
        assert_eq!(&dsi.get_value("objects.type", i).as_string(), want, "objects.type perm {}", i);
    }

    let expected_other_types = ["obj1", "obj2", "obj2", "obj1", "obj2", "obj2"];
    for (i, want) in expected_other_types.iter().enumerate() {
        assert_eq!(
            &dsi.get_value("otherObjects.type", i).as_string(),
            want,
            "otherObjects.type perm {}",
            i
        );
    }

    let expected_nums: [Option<i64>; 6] = [None, Some(1), Some(2), None, Some(1), Some(2)];
    for (i, want) in expected_nums.iter().enumerate() {
        let v = dsi.get_value("otherObjects.ch.n", i);
        match want {
            None => assert!(v.is_null(), "perm {} should be null, got {:?}", i, v),
            Some(n) => {
                assert!(v.is_numeric(), "perm {} should be numeric", i);
                assert_eq!(v.as_numeric().unwrap().to_string(), n.to_string());
            }
        }
    }
}

#[test]
fn test_dataset_index_4b() {
    let json = read_resource("test-data-set-4.json");
    let dsi = DataSetIndex::index(&json);
    assert_eq!(dsi.number_of_permutations(), 24);
}

#[test]
fn test_variant_json_roundtrip() {
    let v = Variant::from_str("hello");
    assert_eq!(variant_to_json(&v), serde_json::json!("hello"));
    let v = Variant::from_i32(42);
    let as_json = variant_to_json(&v);
    assert_eq!(variant_from_json(&as_json).as_numeric().unwrap().to_string(), "42");
    let v = Variant::Array(vec![Variant::from_i32(1), Variant::from_str("x")]);
    let j = variant_to_json(&v);
    assert!(j.is_array());
}
