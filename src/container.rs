//! Variable containers, mirroring `VariantContainer` / `DefaultVariantContainer`
//! / `LocalVariantContainer`.

use crate::variant::Variant;
use std::collections::HashMap;

/// A store for named variables, mirroring the Java `VariantContainer` interface.
/// Default-implemented methods handle the array-indexing overloads in Java.
pub trait VariantContainer {
    fn set_variant(&mut self, name: &str, value: Variant);
    fn get_variant(&self, name: &str) -> Variant;
    fn set_indexed_variant(&mut self, name: &str, index: usize, value: Variant);
    fn get_indexed_variant(&self, name: &str, index: usize) -> Variant;
    fn contains(&self, name: &str) -> bool;
}

/// Hashmap-backed variable container, mirroring `DefaultVariantContainer`.
#[derive(Debug, Default, Clone)]
pub struct DefaultVariantContainer {
    map: HashMap<String, Variant>,
}

impl DefaultVariantContainer {
    pub fn new() -> Self {
        Default::default()
    }
}

impl VariantContainer for DefaultVariantContainer {
    fn set_variant(&mut self, name: &str, value: Variant) {
        self.map.insert(name.to_string(), value);
    }

    fn get_variant(&self, name: &str) -> Variant {
        self.map.get(name).cloned().unwrap_or(Variant::Null)
    }

    fn set_indexed_variant(&mut self, name: &str, index: usize, value: Variant) {
        let existing = self.map.get(name).cloned().unwrap_or(Variant::Null);
        let updated = Variant::set_array_item(existing, index, value);
        self.map.insert(name.to_string(), updated);
    }

    fn get_indexed_variant(&self, name: &str, index: usize) -> Variant {
        let existing = self.map.get(name);
        match existing {
            Some(v) => Variant::get_array_item(v, index),
            None => Variant::Null,
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

/// Locally scoped variable container used for user-defined function invocation,
/// mirroring Java `LocalVariantContainer` (falls back to a delegate container).
pub struct LocalVariantContainer<'a> {
    local: DefaultVariantContainer,
    delegate: &'a dyn VariantContainer,
}

impl<'a> LocalVariantContainer<'a> {
    pub fn new(delegate: &'a dyn VariantContainer) -> Self {
        LocalVariantContainer {
            local: DefaultVariantContainer::new(),
            delegate,
        }
    }
}

impl<'a> VariantContainer for LocalVariantContainer<'a> {
    fn set_variant(&mut self, name: &str, value: Variant) {
        self.local.set_variant(name, value);
    }

    fn get_variant(&self, name: &str) -> Variant {
        if self.local.contains(name) {
            self.local.get_variant(name)
        } else {
            self.delegate.get_variant(name)
        }
    }

    fn set_indexed_variant(&mut self, name: &str, index: usize, value: Variant) {
        self.local.set_indexed_variant(name, index, value);
    }

    fn get_indexed_variant(&self, name: &str, index: usize) -> Variant {
        if self.local.contains(name) {
            self.local.get_indexed_variant(name, index)
        } else {
            self.delegate.get_indexed_variant(name, index)
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.local.contains(name) || self.delegate.contains(name)
    }
}
