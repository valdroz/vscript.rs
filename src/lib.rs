//! # vscript
//!
//! Rust port of [valdroz/vscript](https://github.com/valdroz/vscript) —
//! a language for dynamic execution of mathematical expressions, assignments,
//! array operations, string concatenation, date math and user-defined functions.
//!
//! # Usage
//!
//! ```
//! use vscript::{EquationEval, DefaultVariantContainer, VariantContainer, Variant};
//!
//! // Simple numeric expression
//! let eval = EquationEval::new("10 * 2 + 3").unwrap();
//! let mut container = DefaultVariantContainer::new();
//! let result = eval.eval(&mut container).unwrap();
//! assert_eq!(result.as_string(), "23");
//!
//! // Variables, assignment and a built-in function
//! let eval = EquationEval::new("a = 2; b = 7; pow(a, b) + avg(a, b)").unwrap();
//! let mut container = DefaultVariantContainer::new();
//! let result = eval.eval(&mut container).unwrap();
//! ```

pub mod configuration;
pub mod constants;
pub mod container;
pub mod error;
pub mod json;
pub mod node;
pub mod run_block;
pub mod tracer;
pub mod variant;

pub use container::{DefaultVariantContainer, LocalVariantContainer, VariantContainer};
pub use error::{EvaluationError, VResult};
pub use node::{set_current_time_ms, EquationEval, EquationParser, Node, NodeStats};
pub use run_block::{AbstractFunction, DefaultRunBlock, NoopRunBlock, RunBlock};
pub use tracer::{TraceListener, Tracer};
pub use variant::Variant;
