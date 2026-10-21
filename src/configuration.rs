//! Global configuration, mirroring Java `Configuration`.
//!
//! The Java implementation uses mutable static fields. In Rust we use
//! atomics / a `RwLock` so configuration is safe to read/write across threads.

use rust_decimal::RoundingStrategy;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::RwLock;

static DECIMAL_SCALE: AtomicI32 = AtomicI32::new(3);
static CASE_SENSITIVE: AtomicBool = AtomicBool::new(true);

/// The original uses `RoundingMode.HALF_EVEN` by default.
static ROUNDING_MODE: RwLock<RoundingStrategy> = RwLock::new(RoundingStrategy::MidpointNearestEven);

/// The expression used when parsing an empty/blank source string.
static EMPTY_EVAL_EXPRESSION: RwLock<String> = RwLock::new(String::new());

fn empty_eval_cell() -> &'static RwLock<String> {
    &EMPTY_EVAL_EXPRESSION
}

/// Mirrors `Configuration.setDecimalScale`; returns the previous value.
pub fn set_decimal_scale(scale: i32) -> i32 {
    DECIMAL_SCALE.swap(scale, Ordering::SeqCst)
}

pub fn decimal_scale() -> i32 {
    DECIMAL_SCALE.load(Ordering::SeqCst)
}

/// Mirrors `Configuration.setRoundingMode`; returns the previous mode.
pub fn set_rounding_mode(mode: RoundingStrategy) -> RoundingStrategy {
    let mut w = ROUNDING_MODE.write().unwrap();
    let prev = *w;
    *w = mode;
    prev
}

pub fn rounding_mode() -> RoundingStrategy {
    *ROUNDING_MODE.read().unwrap()
}

/// Mirrors `Configuration.setCaseSensitive`; returns the previous value.
pub fn set_case_sensitive(cs: bool) -> bool {
    CASE_SENSITIVE.swap(cs, Ordering::SeqCst)
}

pub fn is_case_sensitive() -> bool {
    CASE_SENSITIVE.load(Ordering::SeqCst)
}

/// Mirrors `Configuration.setExpressionForEmptyEval`; returns previous value.
/// Ignores `None`/blank expressions just like the Java version.
pub fn set_expression_for_empty_eval(new_expression: Option<String>) -> String {
    let mut w = empty_eval_cell().write().unwrap();
    let prev = w.clone();
    if let Some(expr) = new_expression {
        if !expr.trim().is_empty() {
            *w = expr;
        }
    }
    prev
}

pub fn expression_for_empty_eval() -> String {
    let r = empty_eval_cell().read().unwrap().clone();
    if r.is_empty() {
        "null".to_string()
    } else {
        r
    }
}
