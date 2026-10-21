//! User-defined function resolution, mirroring Java `AbstractFunction`,
//! `RunBlock`, and `DefaultRunBlock`.

use crate::container::VariantContainer;
use crate::error::{EvaluationError, VResult};
use crate::node::Node;
use crate::variant::Variant;
use std::collections::HashMap;
use std::sync::Arc;

/// A user-defined function, equivalent to `AbstractFunction`.
///
/// The Java class parses its name/parameter names from the signature string
/// `name(param1, param2)` and declares an abstract `execute`. Here the `body`
/// is a closure that receives a [`VariantContainer`] of arguments.
pub struct AbstractFunction {
    name: String,
    parameter_names: Vec<String>,
    body: Arc<dyn Fn(&mut dyn VariantContainer) -> VResult<Variant> + Send + Sync>,
}

impl AbstractFunction {
    /// `signature` like `mult(input1, input2)`.
    pub fn new<F>(signature: &str, body: F) -> VResult<Self>
    where
        F: Fn(&mut dyn VariantContainer) -> VResult<Variant> + Send + Sync + 'static,
    {
        let tokens: Vec<&str> = signature.trim().split(['(', ')']).collect();
        if tokens.is_empty() {
            return Err(EvaluationError::new(format!(
                "Invalid function signature \"{}\"",
                signature
            )));
        }
        let name = tokens[0].to_string();
        let mut parameter_names = Vec::new();
        if tokens.len() > 1 {
            for pn in tokens[1].trim().split(',') {
                let trimmed = pn.trim().to_string();
                if !trimmed.is_empty() {
                    parameter_names.push(trimmed);
                }
            }
        }
        Ok(AbstractFunction {
            name,
            parameter_names,
            body: Arc::new(body),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn parameter_names(&self) -> &[String] {
        &self.parameter_names
    }

    pub fn execute(&self, container: &mut dyn VariantContainer) -> VResult<Variant> {
        (self.body)(container)
    }
}

/// Mirror of `RunBlock` — resolves externally defined functions by name and
/// executes nested blocks. Implemented as a trait for parity with Java, but
/// the default concrete type is [`DefaultRunBlock`].
pub trait RunBlock {
    fn execute(&self, container: &mut dyn VariantContainer) -> VResult<Variant>;
    fn resolve_function(&self, name: &str) -> Option<Arc<AbstractFunction>>;
}

/// Default function resolver container. Functions can be resolved and nested
/// RunBlocks executed by it.
#[derive(Default)]
pub struct DefaultRunBlock {
    parent: Option<Box<dyn RunBlock>>,
    runtime_blocks: Vec<Node>,
    functions: HashMap<String, Arc<AbstractFunction>>,
}

impl DefaultRunBlock {
    pub fn new() -> Self {
        Default::default()
    }

    /// `withRunBlock` — append a nested execution block (parsed equation node).
    pub fn with_run_block(mut self, runtime_block: Node) -> Self {
        self.runtime_blocks.push(runtime_block);
        self
    }

    /// Register a function object.
    pub fn register_function(&mut self, func: AbstractFunction) {
        self.functions.insert(func.name().to_string(), Arc::new(func));
    }

    /// Register a function from a signature and a closure, mirroring Java
    /// `registerFunction(String signature, Function<VariantContainer, Variant> body)`.
    pub fn register_closure<F>(&mut self, signature: &str, body: F) -> VResult<()>
    where
        F: Fn(&mut dyn VariantContainer) -> VResult<Variant> + Send + Sync + 'static,
    {
        let f = AbstractFunction::new(signature, body)?;
        self.register_function(f);
        Ok(())
    }
}

impl RunBlock for DefaultRunBlock {
    fn execute(&self, container: &mut dyn VariantContainer) -> VResult<Variant> {
        let mut last_result = Variant::Null;
        for block in &self.runtime_blocks {
            // In Java, runBlock.execute would propagate resolution through
            // `parentRunBlock` set on the nodes. We achieve it by evaluating with
            // `self` as resolver.
            last_result = block.eval(self, container)?;
        }
        Ok(last_result)
    }

    fn resolve_function(&self, name: &str) -> Option<Arc<AbstractFunction>> {
        if let Some(f) = self.functions.get(name) {
            return Some(f.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.resolve_function(name);
        }
        None
    }
}

/// An empty resolver used when evaluating a bare equation without a RunBlock.
pub struct NoopRunBlock;

impl RunBlock for NoopRunBlock {
    fn execute(&self, _container: &mut dyn VariantContainer) -> VResult<Variant> {
        Ok(Variant::Null)
    }

    fn resolve_function(&self, _name: &str) -> Option<Arc<AbstractFunction>> {
        None
    }
}
