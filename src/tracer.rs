//! Evaluation tracing — mirror of Java `TraceListener`, `Tracer`,
//! `TracingBaseNode` and `TracingConstantNode`.
//!
//! When a trace listener is attached, every node's evaluation result is
//! formatted into a human-readable message and reported to the listener — and,
//! as in the original, **an error raised while evaluating a node is swallowed,
//! reported via the listener as `ERROR: <message>`, and that node's value
//! becomes `null`.**
//!
//! ```java
//! // Original Java semantic (TracingBaseNode.execute)
//! try {
//!     Variant result = super.execute(variantContainer);
//!     ...
//!     return result;
//! } catch (Exception ex) {
//!     tracer.trace("ERROR: " + ex.getMessage());
//!     return Variant.nullVariant();
//! }
//! ```

use crate::variant::Variant;
use std::collections::HashMap;

/// Listener receiving trace messages, mirroring the Java `TraceListener`
/// interface.
pub trait TraceListener {
    fn trace(&self, message: &str);
}

impl<F: Fn(&str)> TraceListener for F {
    fn trace(&self, message: &str) {
        self(message)
    }
}

/// Internal tracer holding per-node results for message formatting.
/// Mirrors Java `Tracer`.
#[derive(Default)]
pub struct Tracer<'a> {
    listener: Option<&'a dyn TraceListener>,
    node_results: HashMap<String, Variant>,
}

impl<'a> Tracer<'a> {
    pub fn new(listener: &'a dyn TraceListener) -> Self {
        Tracer {
            listener: Some(listener),
            node_results: HashMap::new(),
        }
    }

    pub fn put(&mut self, node_id: &str, result: Variant) {
        self.node_results.insert(node_id.to_string(), result);
    }

    pub fn get(&self, node_id: &str) -> Variant {
        self.node_results
            .get(node_id)
            .cloned()
            .unwrap_or(Variant::Null)
    }

    pub fn trace(&self, message: String) {
        if let Some(l) = self.listener {
            l.trace(&message);
        }
    }
}
