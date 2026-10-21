//! JSON integration, mirroring the Java `org.valdroz.vscript.json` package:
//! `JsonVariantContainer`, `JsonDataSetMaker`, `DataSetIndex` and `JsonUtils`.
//!
//! Where the Java code uses Gson's `JsonObject`/`JsonArray`/`JsonPrimitive`,
//! this module uses [`serde_json::Value`].

use crate::container::VariantContainer;
use crate::variant::Variant;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Conversion helpers (mirror of JsonUtils.toJsonElement + variantFromPrimitive)
// ---------------------------------------------------------------------------

/// Convert a Variant into a serde_json `Value` (mirror of `JsonUtils.toJsonElement`).
pub fn variant_to_json(value: &Variant) -> Value {
    match value {
        Variant::Null => Value::Null,
        Variant::String(s) => Value::String(s.clone()),
        Variant::Boolean(b) => Value::Bool(*b),
        Variant::Numeric(d) => decimal_to_json_number(d),
        Variant::Array(arr) => Value::Array(arr.iter().map(variant_to_json).collect()),
    }
}

fn decimal_to_json_number(d: &Decimal) -> Value {
    // Preserve arbitrary precision by parsing the textual representation
    // (serde_json arbitrary_precision keeps exactness through the string form).
    let s = d.to_string();
    serde_json::from_str(&s).unwrap_or_else(|_| {
        // Fall back to f64 if parsing fails for any reason.
        match d.to_string().parse::<f64>() {
            Ok(f) => serde_json::json!(f),
            Err(_) => Value::Null,
        }
    })
}

/// Convert a primitive JSON value into a Variant (mirror of `variantFromPrimitive`).
/// Non-primitives (object/array) yield null in the Java implementation's
/// `variantFromPrimitive`; callers handle arrays separately.
pub fn variant_from_json(value: &Value) -> Variant {
    match value {
        Value::Null => Variant::Null,
        Value::String(s) => Variant::String(s.clone()),
        Value::Bool(b) => Variant::Boolean(*b),
        Value::Number(n) => number_to_variant(n),
        // Java variantFromPrimitive returns null for non-primitives
        _ => Variant::Null,
    }
}

fn number_to_variant(n: &serde_json::Number) -> Variant {
    let s = n.to_string();
    match s.parse::<Decimal>() {
        Ok(d) => Variant::Numeric(d),
        Err(_) => Variant::Null,
    }
}

// ---------------------------------------------------------------------------
// JsonUtils
// ---------------------------------------------------------------------------

/// Mirror of `JsonUtils.set(jo, path, value)` — set a dotted path into an object.
pub fn set_path(jo: &mut Value, path: &str, value: Value) {
    set_json_object_path_element(jo, path, value);
}

/// Parse a path element possibly containing an array index, e.g. `arr[2]`
/// into `(Some(2), "arr")`; otherwise `(None, path)`.
fn get_path_index(path: &str) -> (Option<usize>, String) {
    match path.split_once('[') {
        None => (None, path.to_string()),
        Some((prefix, rest)) => {
            let idx_str = rest.split(']').next().unwrap_or("");
            let idx = idx_str
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("Cannot understand path element \"{}\"", path));
            (Some(idx), prefix.to_string())
        }
    }
}

/// Mirror of `JsonUtils.setJsonObjectPathElement`.
fn set_json_object_path_element(jo: &mut Value, path: &str, value: Value) {
    let mut parts = path.splitn(2, '.');
    let head = parts.next().unwrap();
    let rest = parts.next();

    let (idx, head_name) = get_path_index(head);

    match rest {
        None => {
            // leaf assignment
            match idx {
                None => {
                    if let Value::Object(map) = jo {
                        map.insert(head_name, value);
                    }
                }
                Some(i) => {
                    // Ensure array exists at head_name with at least i+1 slots
                    let arr_value = jo
                        .as_object_mut()
                        .and_then(|m| m.get_mut(&head_name));
                    match arr_value {
                        Some(v @ Value::Array(_)) => {
                            let arr = v.as_array_mut().unwrap();
                            while arr.len() <= i {
                                arr.push(Value::Null);
                            }
                            arr[i] = value;
                        }
                        _ => {
                            // create the array or replace the non-array leaf
                            if let Value::Object(map) = jo {
                                let mut arr = match map.remove(&head_name) {
                                    Some(Value::Array(a)) => a,
                                    _ => Vec::new(),
                                };
                                while arr.len() <= i {
                                    arr.push(Value::Null);
                                }
                                arr.insert(i, value);
                                // arr[i] now holds correct; ensure set correctly
                                if arr.len() > i + 1 {
                                    arr.remove(i + 1); // remove the shifted one if insert pushed
                                }
                                map.insert(head_name, Value::Array(arr));
                            }
                        }
                    }
                }
            }
        }
        Some(rest_path) => match idx {
            None => {
                // Ensure object at head_name, then recurse
                if let Value::Object(map) = jo {
                    if !matches!(map.get(&head_name), Some(Value::Object(_))) {
                        map.insert(head_name.clone(), Value::Object(serde_json::Map::new()));
                    }
                    if let Some(child) = map.get_mut(&head_name) {
                        set_json_object_path_element(child, rest_path, value);
                    }
                }
            }
            Some(i) => {
                // Ensure array of objects at head_name, index i, then recurse
                if let Value::Object(map) = jo {
                    if !matches!(map.get(&head_name), Some(Value::Array(_))) {
                        map.insert(head_name.clone(), Value::Array(Vec::new()));
                    }
                    if let Some(Value::Array(arr)) = map.get_mut(&head_name) {
                        while arr.len() <= i {
                            arr.push(Value::Object(serde_json::Map::new()));
                        }
                        set_json_object_path_element(&mut arr[i], rest_path, value);
                    }
                }
            }
        },
    }
}

/// Mirror of `JsonUtils.merge`.
pub fn merge(target: &Value, from: &Value) -> Value {
    let mut result = target.clone();
    if let (Value::Object(res_map), Value::Object(from_map)) = (&mut result, from) {
        for (key, je2) in from_map {
            let merged = match (res_map.get(key), je2) {
                (Some(Value::Object(a)), Value::Object(_)) => Some(merge(&Value::Object(a.clone()), je2)),
                (Some(Value::Array(a)), Value::Array(b)) => {
                    let mut arr = a.clone();
                    arr.extend(b.iter().cloned());
                    Some(Value::Array(arr))
                }
                _ => Some(je2.clone()),
            };
            if let Some(v) = merged {
                res_map.insert(key.clone(), v);
            }
        }
    }
    result
}

/// Convert a JSON object into a `Map<String, serde_json::Value>`.
pub fn to_map(jo: &Value) -> Option<serde_json::Map<String, Value>> {
    jo.as_object().cloned()
}

// ---------------------------------------------------------------------------
// JsonVariantContainer
// ---------------------------------------------------------------------------

/// Variable container backed by a (flattened) JSON object — mirror of
/// Java `JsonVariantContainer`.
#[derive(Debug, Clone)]
pub struct JsonVariantContainer {
    pub json: Value,
}

impl JsonVariantContainer {
    pub fn new(json: Value) -> Self {
        JsonVariantContainer { json }
    }

    /// Mirror of `JsonVariantContainer.jsonToVariantContainers` — flatten a JSON
    /// object into a set of variant containers (one per permutation).
    pub fn json_to_variant_containers(json: &Value, modes: &[DataSetMakerMode]) -> Vec<JsonVariantContainer> {
        let maker = JsonDataSetMaker::new(json, "", modes);
        maker
            .get_data_sets()
            .into_iter()
            .map(JsonVariantContainer::new)
            .collect()
    }

    pub fn json(&self) -> &Value {
        &self.json
    }

    /// Mirror of `JsonVariantContainer.getJsonElement(name)` — assemble an object
    /// from dotted prefix entries if a direct key lookup fails.
    pub fn get_json_element(&self, name: &str) -> Value {
        if let Some(v) = self.json.get(name) {
            return v.clone();
        }
        let path_pref = format!("{}.", name);
        let mut assembly = serde_json::Map::new();
        if let Value::Object(map) = &self.json {
            for (key, value) in map {
                if key == name || key.starts_with(&path_pref) {
                    let sub_path = &key[name.len() + 1..];
                    let mut jo = Value::Object(std::mem::take(&mut assembly));
                    set_json_object_path_element(&mut jo, sub_path, value.clone());
                    assembly = jo.as_object().cloned().unwrap_or_default();
                }
            }
        }
        if assembly.is_empty() {
            Value::Null
        } else {
            Value::Object(assembly)
        }
    }
}

impl VariantContainer for JsonVariantContainer {
    fn set_variant(&mut self, name: &str, value: Variant) {
        if let Value::Object(map) = &mut self.json {
            map.insert(name.to_string(), variant_to_json(&value));
        }
    }

    fn get_variant(&self, name: &str) -> Variant {
        let je = match self.json.get(name) {
            Some(v) => v,
            None => return Variant::Null,
        };
        match je {
            Value::Array(arr) => Variant::Array(arr.iter().map(variant_from_json).collect()),
            primitive => variant_from_json(primitive),
        }
    }

    fn set_indexed_variant(&mut self, name: &str, index: usize, value: Variant) {
        if let Value::Object(map) = &mut self.json {
            let existing = map.remove(name);
            let mut arr = match existing {
                Some(Value::Array(a)) => a,
                Some(other) => vec![other],
                None => Vec::new(),
            };
            while arr.len() <= index {
                arr.push(Value::Null);
            }
            if arr.len() > index {
                arr[index] = variant_to_json(&value);
            } else {
                arr.insert(index, variant_to_json(&value));
            }
            map.insert(name.to_string(), Value::Array(arr));
        }
    }

    fn get_indexed_variant(&self, name: &str, index: usize) -> Variant {
        match self.json.get(name) {
            Some(Value::Array(arr)) => arr.get(index).map(variant_from_json).unwrap_or(Variant::Null),
            _ => Variant::Null,
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.json.get(name).is_some()
    }
}

// ---------------------------------------------------------------------------
// JsonDataSetMaker
// ---------------------------------------------------------------------------

/// Operation modes, mirror of `JsonDataSetMaker.Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSetMakerMode {
    KeepArraysForPrimitives,
    KeepComplexArrays,
    AllFlattened,
}

/// Flattens a JSON document into a set of flat "data set" objects — mirror of
/// Java `JsonDataSetMaker`. Arrays induce cross-product permutations.
pub struct JsonDataSetMaker {
    root: JfObject,
}

#[derive(Debug, Default)]
struct JfObject {
    name: String,
    // children grouped by name, in insertion order
    children: Vec<(String, JfObject)>,
    flats: Vec<Value>,
}

impl JfObject {
    fn new(name: &str) -> Self {
        JfObject {
            name: name.to_string(),
            children: Vec::new(),
            flats: vec![Value::Object(serde_json::Map::new())],
        }
    }

    fn add_child(&mut self, child: JfObject) {
        let name = child.name.clone();
        self.children.push((name, child));
    }

    fn add_node(&mut self, key: &str, value: Value) {
        let mut additions: Vec<Value> = Vec::new();
        for jo in &self.flats {
            if jo.get(key).is_some() {
                // key exists: keep original plus a copy-with-replacement permutation
                additions.push(jo.clone());
                let mut addition = jo.clone();
                if let Value::Object(map) = &mut addition {
                    map.insert(key.to_string(), value.clone());
                }
                additions.push(addition);
            } else {
                let mut updated = jo.clone();
                if let Value::Object(map) = &mut updated {
                    map.insert(key.to_string(), value.clone());
                }
                additions.push(updated);
            }
        }
        // Java used Sets.newHashSet(): identical permutations are deduplicated.
        dedup_values(&mut additions);
        self.flats = additions;
    }

    fn add_array(&mut self, key: &str, array: Value) {
        let mut additions: Vec<Value> = Vec::new();
        for jo in &self.flats {
            if jo.get(key).is_some() {
                additions.push(jo.clone());
                let mut addition = jo.clone();
                if let Value::Object(map) = &mut addition {
                    map.insert(key.to_string(), array.clone());
                }
                additions.push(addition);
            } else {
                let mut updated = jo.clone();
                if let Value::Object(map) = &mut updated {
                    map.insert(key.to_string(), array.clone());
                }
                additions.push(updated);
            }
        }
        dedup_values(&mut additions);
        self.flats = additions;
    }

    fn add_node_as_array(&mut self, key: &str, value: Value) {
        let mut additions: Vec<Value> = Vec::new();
        for jo in &self.flats {
            let mut updated = jo.clone();
            if let Value::Object(map) = &mut updated {
                match map.get_mut(key) {
                    Some(Value::Array(arr)) => arr.push(value.clone()),
                    Some(existing) => {
                        let prev = existing.clone();
                        *existing = Value::Array(vec![prev, value.clone()]);
                    }
                    None => {
                        map.insert(key.to_string(), Value::Array(vec![value.clone()]));
                    }
                }
            }
            additions.push(updated);
        }
        dedup_values(&mut additions);
        self.flats = additions;
    }

    fn get_data_sets(&self) -> Vec<Value> {
        let mut data_set = self.flats.clone();
        if !self.children.is_empty() {
            // Group children by name, then combine each group's data sets
            let mut grouped: Vec<(String, Vec<&JfObject>)> = Vec::new();
            for (name, child) in &self.children {
                if let Some((_, group)) = grouped.iter_mut().find(|(n, _)| n == name) {
                    group.push(child);
                } else {
                    grouped.push((name.clone(), vec![child]));
                }
            }
            let mut augmented_child_data_set: Vec<Value> = Vec::new();
            for (_, group) in grouped {
                let mut list: Vec<Value> = Vec::new();
                for child in group {
                    list.extend(child.get_data_sets());
                }
                augmented_child_data_set = replicate(augmented_child_data_set, list);
            }
            data_set = replicate(data_set, augmented_child_data_set);
        }
        data_set
    }
}

/// Remove duplicate JSON objects (Java used Sets.newHashSet()).
fn dedup_values(values: &mut Vec<Value>) {
    let mut seen = std::collections::HashSet::new();
    values.retain(|v| seen.insert(canonical_json_string(v)));
}

/// Deterministic serialization for dedup comparison.
fn canonical_json_string(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", k, canonical_json_string(&map[k])))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(canonical_json_string).collect();
            format!("[{}]", inner.join(","))
        }
        other => other.to_string(),
    }
}

/// Cartesian-product merge of two sets of flat objects — mirror of
/// `JsonDataSetMaker.replicate`.
fn replicate(base: Vec<Value>, source: Vec<Value>) -> Vec<Value> {
    if base.is_empty() {
        return source;
    }
    let mut res = Vec::new();
    for jo1 in &base {
        for jo2 in &source {
            let size1 = jo1.as_object().map(|m| m.len()).unwrap_or(0);
            let size2 = jo2.as_object().map(|m| m.len()).unwrap_or(0);
            if size1 == 0 {
                res.push(jo2.clone());
            } else if size2 == 0 {
                res.push(jo1.clone());
            } else {
                let mut merged = serde_json::Map::new();
                if let Value::Object(m1) = jo1 {
                    for (k, v) in m1 {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                if let Value::Object(m2) = jo2 {
                    for (k, v) in m2 {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                res.push(Value::Object(merged));
            }
        }
    }
    res
}

impl JsonDataSetMaker {
    pub fn new(json: &Value, prefix: &str, modes: &[DataSetMakerMode]) -> Self {
        let keep_array_for_primitives = modes.contains(&DataSetMakerMode::KeepArraysForPrimitives);
        let keep_complex_array = modes.contains(&DataSetMakerMode::KeepComplexArrays);
        let mut maker = JsonDataSetMaker {
            root: JfObject::new(""),
        };
        flatten(
            &mut maker.root,
            prefix,
            json,
            keep_array_for_primitives,
            keep_complex_array,
        );
        maker
    }

    pub fn get_data_sets(&self) -> Vec<Value> {
        self.root.get_data_sets()
    }
}

fn new_key(pref: &str, key: &str) -> String {
    if pref.trim().is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", pref, key)
    }
}

fn flatten(
    parent: &mut JfObject,
    pref: &str,
    je: &Value,
    keep_array_for_primitives: bool,
    keep_complex_array: bool,
) {
    match je {
        Value::Object(map) => {
            let idx = parent.children.len();
            parent.add_child(JfObject::new(pref));
            let mut child = parent.children[idx].1.clone_shallow();
            for (k, v) in map {
                let key = new_key(pref, k);
                flatten(&mut child, &key, v, keep_array_for_primitives, keep_complex_array);
            }
            parent.children[idx].1 = child;
        }
        Value::Array(arr) => {
            for item in arr {
                if item.is_object() || item.is_array() {
                    flatten(
                        parent,
                        pref,
                        item,
                        keep_array_for_primitives,
                        keep_complex_array,
                    );
                } else if keep_array_for_primitives {
                    parent.add_node_as_array(pref, item.clone());
                } else {
                    parent.add_node(pref, item.clone());
                }
            }
            if keep_complex_array {
                parent.add_array(pref, je.clone());
            }
        }
        primitive => {
            parent.add_node(pref, primitive.clone());
        }
    }
}

impl JfObject {
    /// Detached shallow copy used when first populating a child JfObject
    /// (avoids aliasing issues while recursing into a child stored in self.children).
    fn clone_shallow(&self) -> JfObject {
        JfObject {
            name: self.name.clone(),
            children: Vec::new(),
            flats: self.flats.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// DataSetIndex
// ---------------------------------------------------------------------------

/// Flattened, indexed JSON dataset with permutation support — mirror of
/// `DataSetIndex`.
#[derive(Debug, Default)]
pub struct DataSetIndex {
    value_map: HashMap<String, VariantValue>,
    sub_indexes: Vec<Vec<DataSetIndex>>,
    json_element_map: HashMap<String, Vec<Value>>,
    size: usize,
}

#[derive(Debug, Clone)]
enum VariantValue {
    Base(Variant),
    Indexed(Vec<Variant>),
}

impl VariantValue {
    fn set_value(&mut self, index: usize, value: Variant) {
        match self {
            VariantValue::Base(v) => *v = value,
            VariantValue::Indexed(vals) => {
                while vals.len() < index + 1 {
                    vals.push(Variant::Null);
                }
                vals[index] = value;
            }
        }
    }

    fn get_value(&self, index: usize) -> Variant {
        match self {
            VariantValue::Base(v) => v.clone(),
            VariantValue::Indexed(vals) => vals.get(index).cloned().unwrap_or(Variant::Null),
        }
    }

    fn size(&self) -> usize {
        match self {
            VariantValue::Base(_) => 1,
            VariantValue::Indexed(vals) => vals.len(),
        }
    }
}

impl Default for VariantValue {
    fn default() -> Self {
        VariantValue::Base(Variant::Null)
    }
}

impl DataSetIndex {
    /// Flatten a JSON object into an indexed data set.
    pub fn index(jo: &Value) -> DataSetIndex {
        let mut dsi = DataSetIndex {
            size: 1,
            ..Default::default()
        };
        populate_index(&mut dsi, "", jo);
        dsi.consolidate();
        dsi
    }

    pub fn number_of_permutations(&self) -> usize {
        self.size
    }

    pub fn get_value(&self, name: &str, permutation_index: usize) -> Variant {
        self.value_map
            .get(name)
            .map(|v| v.get_value(permutation_index))
            .unwrap_or(Variant::Null)
    }

    pub fn has_permutations(&self, name: &str) -> bool {
        self.value_map
            .get(name)
            .map(|v| v.size() > 1)
            .unwrap_or(false)
    }

    pub fn get_json_element(&self, name: &str, permutation_index: usize) -> Value {
        if let Some(elements) = self.json_element_map.get(name) {
            return elements
                .get(permutation_index)
                .cloned()
                .unwrap_or(Value::Null);
        }
        let pref = format!("{}.", name);
        let mut jo = Value::Object(serde_json::Map::new());
        for key in self.value_map.keys().filter(|k| k.starts_with(&pref)) {
            let value = self.get_value(key, permutation_index);
            set_path(&mut jo, &key[pref.len()..], variant_to_json(&value));
        }
        jo
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.value_map.keys()
    }

    pub fn object_array_names(&self) -> impl Iterator<Item = &String> {
        self.json_element_map.keys()
    }

    pub fn as_variant_container(&self, permutation_index: usize) -> DataSetVariantContainer<'_> {
        DataSetVariantContainer {
            index: self,
            permutation_index,
        }
    }

    fn consolidate(&mut self) {
        let mut permutations: Vec<DataSetIndex> = Vec::new();
        if !self.sub_indexes.is_empty() {
            generate_permutations(&self.sub_indexes, &mut permutations, 0, DataSetIndex::empty());
        }
        let mut level = 0;
        for dsi in permutations {
            let inc = dsi.number_of_permutations().max(1);
            for (key, v) in dsi.value_map {
                for j in 0..inc {
                    self.set_index_value(&key, level + j, v.get_value(j));
                }
            }
            for (key, jvalues) in dsi.json_element_map {
                for j in 0..inc {
                    let value = jvalues
                        .get(j)
                        .cloned()
                        .or_else(|| {
                            if jvalues.len() == 1 {
                                jvalues.first().cloned()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(Value::Null);
                    self.set_index_json_value(&key, level + j, value);
                }
            }
            level += inc;
        }
        self.size = level.max(1);
    }

    fn empty() -> DataSetIndex {
        DataSetIndex {
            size: 1,
            ..Default::default()
        }
    }

    fn set_index_value(&mut self, key: &str, index: usize, variant: Variant) {
        let entry = self
            .value_map
            .entry(key.to_string())
            .or_insert_with(|| VariantValue::Indexed(Vec::new()));
        if let VariantValue::Base(_) = entry {
            *entry = VariantValue::Indexed(Vec::new());
        }
        entry.set_value(index, variant);
        self.size = self.size.max(entry.size());
    }

    fn set_index_json_value(&mut self, key: &str, index: usize, je: Value) {
        let elements = self.json_element_map.entry(key.to_string()).or_default();
        while elements.len() < index + 1 {
            elements.push(Value::Null);
        }
        elements[index] = je;
    }
}

pub struct DataSetVariantContainer<'a> {
    index: &'a DataSetIndex,
    permutation_index: usize,
}

impl<'a> VariantContainer for DataSetVariantContainer<'a> {
    fn set_variant(&mut self, _name: &str, _value: Variant) {
        // no op (read-only)
    }

    fn get_variant(&self, name: &str) -> Variant {
        self.index.get_value(name, self.permutation_index)
    }

    fn set_indexed_variant(&mut self, _name: &str, _index: usize, _value: Variant) {
        // no op
    }

    fn get_indexed_variant(&self, name: &str, index: usize) -> Variant {
        Variant::get_array_item(&self.index.get_value(name, self.permutation_index), index)
    }

    fn contains(&self, name: &str) -> bool {
        self.index.value_map.contains_key(name)
    }
}

fn populate_index(dsi: &mut DataSetIndex, key: &str, je: &Value) {
    match je {
        Value::Object(map) => {
            for (k, v) in map {
                let new_key = new_key(key, k);
                populate_index(dsi, &new_key, v);
            }
        }
        Value::Array(arr) => {
            let mut indices: Vec<DataSetIndex> = Vec::new();
            let mut variants: Vec<Variant> = Vec::new();
            for item in arr {
                if let Value::Object(_) = item {
                    let mut ndsi = DataSetIndex::empty();
                    populate_index(&mut ndsi, key, item);
                    ndsi.consolidate();
                    indices.push(ndsi);
                } else {
                    variants.push(variant_from_json(item));
                }
            }
            if !variants.is_empty() {
                dsi.value_map
                    .insert(key.to_string(), VariantValue::Base(Variant::Array(variants)));
            }
            if !indices.is_empty() {
                dsi.sub_indexes.push(indices);
                dsi.set_index_json_value(key, 0, je.clone());
            }
        }
        primitive => {
            dsi.value_map
                .insert(key.to_string(), VariantValue::Base(variant_from_json(primitive)));
        }
    }
}

fn generate_permutations(
    all_index: &[Vec<DataSetIndex>],
    permutations: &mut Vec<DataSetIndex>,
    depth: usize,
    dsi: DataSetIndex,
) {
    if all_index.len() == depth {
        permutations.push(dsi);
        return;
    }
    for dsi2 in &all_index[depth] {
        let mut ndsi = DataSetIndex {
            value_map: dsi.value_map.clone(),
            sub_indexes: Vec::new(),
            json_element_map: dsi.json_element_map.clone(),
            size: dsi.number_of_permutations().max(dsi2.number_of_permutations()),
        };
        ndsi.value_map.extend(dsi2.value_map.clone());
        ndsi.json_element_map.extend(dsi2.json_element_map.clone());
        generate_permutations(all_index, permutations, depth + 1, ndsi);
    }
}
