//! AST node types, the recursive-descent parser, and the evaluator —
//! a unified Rust translation of Java `BaseNode`/`ConstantNode`/`CompositeNode` (evaluation)
//! plus `EquationParser` (parsing).

use crate::configuration;
use crate::constants::*;
use crate::container::{LocalVariantContainer, VariantContainer};
use crate::error::{EvaluationError, VResult};
use crate::run_block::RunBlock;
use crate::tracer::Tracer;
use crate::variant::{function_name_from_code, Variant};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};

/// Thread-safe, overridable current-time supplier (millis since epoch), mirroring
/// `BaseNode.currentTime`.
static CURRENT_TIME_MS: AtomicI64 = AtomicI64::new(0);
static USE_REAL_TIME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

fn current_time_ms() -> i64 {
    if USE_REAL_TIME.load(AtomicOrdering::SeqCst) {
        Utc::now().timestamp_millis()
    } else {
        CURRENT_TIME_MS.load(AtomicOrdering::SeqCst)
    }
}

/// Set a fixed current time used by time-sensitive built-ins (`now()`, `day()`,
/// `year()`, `days_before_now()`, ...). Mirrors `EquationEval.setCurrentTimeSupplier()`.
/// Returns the previous override value (`None` = real time was active).
pub fn set_current_time_ms(ms: Option<i64>) -> Option<i64> {
    match ms {
        Some(value) => {
            USE_REAL_TIME.store(false, AtomicOrdering::SeqCst);
            let prev = CURRENT_TIME_MS.swap(value, AtomicOrdering::SeqCst);
            let had_override = !USE_REAL_TIME.load(AtomicOrdering::SeqCst);
            let prev_opt = if had_override { Some(prev) } else { None };
            prev_opt
        }
        None => {
            let prev = CURRENT_TIME_MS.load(AtomicOrdering::SeqCst);
            USE_REAL_TIME.store(true, AtomicOrdering::SeqCst);
            Some(prev)
        }
    }
}

fn now_datetime() -> DateTime<Utc> {
    Utc.timestamp_millis_opt(current_time_ms()).unwrap()
}

// ---------------------------------------------------------------
// AST
// ---------------------------------------------------------------

/// A node in the parsed AST. Because Java used mutable subclass fields with
/// shared constant instances, we model nodes as an owned tree; shared constant
/// nodes are created fresh at parse time.
#[derive(Debug, Clone)]
pub enum NodeKind {
    /// An operator/function/composite node whose outcome depends on children.
    Op(OpNode),
    /// A literal constant.
    Const(Variant),
}

/// An operation node — corresponds to a non-constant `BaseNode`.
#[derive(Debug, Clone)]
pub struct OpNode {
    pub op: i32,
    /// Node identifier for variable-resolution (Java `name`).
    pub name: String,
    pub left: Option<Box<Node>>,
    pub right: Option<Box<Node>>,
    pub params: Vec<Node>,
    /// `?` fallback expression (Java `valueSubstitution`).
    pub substitution: Option<Box<Node>>,
}

/// A parsed/evaluable AST node (Java `Node`), now an owned tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
}

impl Node {
    fn new_op(op: i32) -> Node {
        Node {
            id: String::new(),
            kind: NodeKind::Op(OpNode {
                op,
                name: String::new(),
                left: None,
                right: None,
                params: Vec::new(),
                substitution: None,
            }),
        }
    }

    fn named_op(op: i32, name: &str) -> Node {
        Node {
            id: String::new(),
            kind: NodeKind::Op(OpNode {
                op,
                name: name.to_string(),
                left: None,
                right: None,
                params: Vec::new(),
                substitution: None,
            }),
        }
    }

    fn constant(value: Variant) -> Node {
        Node {
            id: String::new(),
            kind: NodeKind::Const(value),
        }
    }

    /// Whether this node is a variable or local-variable node.
    fn is_variable_node(&self) -> bool {
        matches!(
            &self.kind,
            NodeKind::Op(o) if o.op == NT_VARIABLE || o.op == NT_LOCAL_VARIABLE
        )
    }

    /// Run the node. The resolver carries user-defined function dispatch
    /// (mirrors the Java pattern where nodes read `parentRunBlock`).
    pub fn eval(&self, resolver: &dyn RunBlock, container: &mut dyn VariantContainer) -> VResult<Variant> {
        self.eval_inner(resolver, container, None)
    }

    /// Evaluation with optional tracer. When a tracer is present, errors from
    /// a node are caught, reported as `ERROR: ...` and the node yields
    /// `Variant::Null` (mirroring Java `TracingBaseNode.execute`).
    pub(crate) fn eval_inner(
        &self,
        resolver: &dyn RunBlock,
        container: &mut dyn VariantContainer,
        tracer: Option<&RefCell<Tracer>>,
    ) -> VResult<Variant> {
        match &self.kind {
            NodeKind::Const(v) => {
                if let Some(t) = tracer {
                    t.borrow_mut().put(&self.id, v.clone());
                }
                Ok(v.clone())
            }
            NodeKind::Op(op_node) => {
                if let Some(t) = tracer {
                    match eval_op(op_node, resolver, container, Some(t)) {
                        Ok(result) => {
                            {
                                let mut tr = t.borrow_mut();
                                tr.put(&self.id, result.clone());
                                let msg = trace_message(op_node, &result, &tr);
                                tr.trace(msg);
                            }
                            Ok(result)
                        }
                        Err(err) => {
                            t.borrow().trace(format!("ERROR: {}", err.message));
                            Ok(Variant::Null)
                        }
                    }
                } else {
                    eval_op(op_node, resolver, container, None)
                }
            }
        }
    }

    /// Mirror of `Node.collectStats`.
    pub fn collect_stats(&self, stats: &mut NodeStats) {
        match &self.kind {
            NodeKind::Const(_) => {}
            NodeKind::Op(o) => {
                match o.op {
                    NT_VARIABLE | NT_LOCAL_VARIABLE => stats.referenced_variable(o.name.clone()),
                    NT_FUNCTION => stats.referenced_ext_function(o.name.clone()),
                    _ => {}
                }
                if let Some(sub) = &o.substitution {
                    sub.collect_stats(stats);
                }
                if let Some(l) = &o.left {
                    l.collect_stats(stats);
                }
                if let Some(r) = &o.right {
                    r.collect_stats(stats);
                }
                for p in &o.params {
                    p.collect_stats(stats);
                }
            }
        }
    }
}

// ---------------------------------------------------------------
// Stats
// ---------------------------------------------------------------

/// Mirror of Java `NodeStats`.
#[derive(Debug, Default, Clone)]
pub struct NodeStats {
    pub variables: std::collections::BTreeSet<String>,
    pub functions: std::collections::BTreeSet<String>,
}

impl NodeStats {
    pub(crate) fn referenced_variable(&mut self, name: String) {
        self.variables.insert(name);
    }
    pub(crate) fn referenced_ext_function(&mut self, name: String) {
        self.functions.insert(name);
    }
    pub fn referenced_variables(&self) -> impl Iterator<Item = &String> {
        self.variables.iter()
    }
    pub fn referenced_ext_functions(&self) -> impl Iterator<Item = &String> {
        self.functions.iter()
    }
}

// ---------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------

fn eval_op(
    op: &OpNode,
    resolver: &dyn RunBlock,
    container: &mut dyn VariantContainer,
    tracer: Option<&RefCell<Tracer>>,
) -> VResult<Variant> {
    let left = |c: &mut dyn VariantContainer| -> VResult<Variant> {
        match &op.left {
            Some(n) => n.eval_inner(resolver, c, tracer),
            None => Ok(Variant::Null),
        }
    };
    let right = |c: &mut dyn VariantContainer| -> VResult<Variant> {
        match &op.right {
            Some(n) => n.eval_inner(resolver, c, tracer),
            None => Ok(Variant::Null),
        }
    };
    let param = |idx: usize, c: &mut dyn VariantContainer| -> VResult<Variant> {
        if let Some(n) = op.params.get(idx) {
            n.eval_inner(resolver, c, tracer)
        } else {
            Ok(Variant::Null)
        }
    };

    match op.op {
        NT_VARIABLE | NT_LOCAL_VARIABLE => {
            // Array element?
            if !op.params.is_empty() {
                let index_v = param(0, container)?;
                let index = index_v
                    .as_numeric()
                    .unwrap_or_default()
                    .to_string()
                    .parse::<i64>()
                    .unwrap_or(0);
                let value = if index >= 0 {
                    container.get_indexed_variant(&op.name, index as usize)
                } else {
                    Variant::Null
                };
                if value.is_null() {
                    if let Some(sub) = &op.substitution {
                        return sub.eval_inner(resolver, container, tracer);
                    }
                }
                return Ok(value);
            }
            // Scalar variable
            let value = container.get_variant(&op.name);
            if value.is_null() {
                if let Some(sub) = &op.substitution {
                    return sub.eval_inner(resolver, container, tracer);
                }
            }
            Ok(value)
        }

        op_code if op_code == '*' as i32 => Ok(left(container)?.multiply(&right(container)?)?),
        op_code if op_code == '+' as i32 => Ok(left(container)?.add(&right(container)?)?),
        op_code if op_code == '-' as i32 => {
            let l = left(container)?;
            if l.is_numeric() || l.is_string() || l.is_array() {
                Ok(l.minus(&right(container)?)?)
            } else {
                // unary negate (matches Java `!`/`neg` behavior)
                Ok(l.negate()?)
            }
        }
        op_code if op_code == '/' as i32 => Ok(left(container)?.divide(&right(container)?)?),
        op_code if op_code == '=' as i32 => {
            // assignment
            let lhs = op.left.as_ref().ok_or_else(|| EvaluationError::new("Invalid assignment"))?;
            let new_value = right(container)?;
            assign_value(lhs, new_value, resolver, container, tracer)
        }
        op_code if op_code == '!' as i32 => {
            let l = left(container)?;
            if !l.is_null() {
                Ok(Variant::Boolean(!l.as_boolean()))
            } else {
                Ok(Variant::Null)
            }
        }
        op_code if op_code == '&' as i32 => {
            let l = left(container)?;
            let r = right(container)?;
            let lv = l.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
            let rv = r.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
            Ok(Variant::from_i64(lv & rv))
        }
        op_code if op_code == '|' as i32 => {
            let l = left(container)?;
            let r = right(container)?;
            let lv = l.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
            let rv = r.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
            Ok(Variant::from_i64(lv | rv))
        }
        op_code if op_code == '^' as i32 => {
            let l = left(container)?;
            let r = right(container)?;
            let lv = l.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
            let rv = r.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
            Ok(Variant::from_i64(lv ^ rv))
        }
        op_code if op_code == '>' as i32 => {
            let l = left(container)?;
            let r = right(container)?;
            Ok(Variant::Boolean(l.compare(&r) == std::cmp::Ordering::Greater))
        }
        op_code if op_code == '<' as i32 => {
            let l = left(container)?;
            let r = right(container)?;
            Ok(Variant::Boolean(l.compare(&r) == std::cmp::Ordering::Less))
        }
        NT_LOP_EQUALS => {
            let l = left(container)?;
            let r = right(container)?;
            Ok(Variant::Boolean(l.value_equals(&r)))
        }
        NT_LOP_NOT_EQUALS => {
            let l = left(container)?;
            let r = right(container)?;
            Ok(Variant::Boolean(!l.value_equals(&r)))
        }
        NT_LOP_MORE_EQUALS => {
            let l = left(container)?;
            let r = right(container)?;
            let cmp = l.compare(&r);
            Ok(Variant::Boolean(cmp == std::cmp::Ordering::Greater || cmp == std::cmp::Ordering::Equal))
        }
        NT_LOP_LESS_EQUALS => {
            let l = left(container)?;
            let r = right(container)?;
            let cmp = l.compare(&r);
            Ok(Variant::Boolean(cmp == std::cmp::Ordering::Less || cmp == std::cmp::Ordering::Equal))
        }
        NT_LOP_AND => {
            let l = left(container)?;
            if l.as_boolean() {
                let r = right(container)?;
                Ok(Variant::Boolean(r.as_boolean()))
            } else {
                Ok(Variant::Boolean(false))
            }
        }
        NT_LOP_OR => {
            let l = left(container)?;
            if l.as_boolean() {
                Ok(Variant::Boolean(true))
            } else {
                let r = right(container)?;
                Ok(Variant::Boolean(r.as_boolean()))
            }
        }

        NT_MF_POWER => {
            let base = param(0, container)?;
            let exp = param(1, container)?;
            base.pow(&exp)
        }

        // ---- Scalar math functions mapped to Variant::mfunc ----
        NT_MF_SIN | NT_MF_COS | NT_MF_ASIN | NT_MF_ACOS | NT_MF_TAN | NT_MF_ATAN
        | NT_MF_EXP | NT_MF_LN | NT_MF_LOG | NT_MF_SQRT | NT_MF_ABS => {
            let arg = param(0, container)?;
            arg.mfunc(op.op)
        }

        NT_MF_FLOOR_MOD => {
            if op.params.len() == 2 {
                let x = param(0, container)?
                    .as_numeric()
                    .unwrap_or_default()
                    .to_string()
                    .parse::<i64>()
                    .unwrap_or(0);
                let y = param(1, container)?
                    .as_numeric()
                    .unwrap_or_default()
                    .to_string()
                    .parse::<i64>()
                    .unwrap_or(0);
                Ok(Variant::from_i64(x.rem_euclid(y)))
            } else {
                Err(EvaluationError::new("Function floor_mod takes two int parameters."))
            }
        }

        NT_MF_NEG => {
            let arg = param(0, container)?;
            arg.negate()
        }

        NT_MF_DAY => {
            let now = now_datetime();
            Ok(Variant::from_i32(now.day() as i32))
        }
        NT_MF_MONTH => {
            let now = now_datetime();
            Ok(Variant::from_i32(now.month() as i32))
        }
        NT_MF_YEAR => {
            let now = now_datetime();
            Ok(Variant::from_i32(now.year()))
        }
        NT_MF_DAY_OF_YEAR => {
            let now = now_datetime();
            Ok(Variant::from_i32(now.ordinal() as i32))
        }
        NT_MF_DAYS_IN_MONTH => {
            let month_offset = param(0, container)?
                .as_numeric()
                .unwrap_or_default()
                .to_string()
                .parse::<i64>()
                .unwrap_or(0);
            let dt = now_datetime();
            // plus months via chrono (manual, since Chrono lacks Months)
            let total_months = dt.year() as i64 * 12 + (dt.month() as i64 - 1) + month_offset;
            let y = (total_months.div_euclid(12)) as i32;
            let m = (total_months.rem_euclid(12) + 1) as u32;
            let dim = days_in_month(y, m);
            Ok(Variant::from_i32(dim as i32))
        }
        NT_MF_ISO => {
            let v = param(0, container)?;
            if !v.is_string() {
                return Err(EvaluationError::new(format!("ISO-8601 formatted string expected. Got: {}", v)));
            }
            let ms = parse_iso_to_millis(&v.as_string())?;
            Ok(Variant::from_i64(ms))
        }
        NT_MF_DAY_OF_WEEK => {
            let v = param(0, container)?;
            // Preserve the source offset like Joda's withOffsetParsed() so the
            // weekday reflects the moment's local day.
            let weekday: i32 = if v.is_string() {
                match DateTime::parse_from_rfc3339(&v.as_string()) {
                    Ok(dt) => dt.weekday().num_days_from_monday() as i32 + 1,
                    Err(_) => {
                        let dt = parse_iso(&v.as_string()).map_err(|_| {
                            EvaluationError::new(format!(
                                "Invalid timestamp. Function day_of_week(x) takes timestamp parameter in String ISO format or numeric milliseconds from 1/1/1970. Provided x: {}",
                                v.as_string()
                            ))
                        })?;
                        dt.weekday().num_days_from_monday() as i32 + 1
                    }
                }
            } else if v.is_numeric() {
                let ms = v
                    .as_numeric()
                    .unwrap_or_default()
                    .to_string()
                    .parse::<i64>()
                    .map_err(|_| {
                        EvaluationError::new(format!(
                            "Invalid timestamp. Function day_of_week(x) takes timestamp parameter in String ISO format or numeric milliseconds from 1/1/1970. Provided x: {}",
                            v.as_string()
                        ))
                    })?;
                let dt = Utc.timestamp_millis_opt(ms).single().ok_or_else(|| {
                    EvaluationError::new(format!(
                        "Invalid timestamp. Function day_of_week(x) takes timestamp parameter in String ISO format or numeric milliseconds from 1/1/1970. Provided x: {}",
                        v.as_string()
                    ))
                })?;
                dt.weekday().num_days_from_monday() as i32 + 1
            } else {
                return Err(EvaluationError::new(format!(
                    "Invalid timestamp. Function day_of_week(x) takes timestamp parameter in String ISO format or numeric milliseconds from 1/1/1970. Provided x: {}",
                    v.as_string()
                )));
            };
            Ok(Variant::from_i32(weekday))
        }
        NT_MF_NOW => Ok(Variant::from_i64(current_time_ms())),
        NT_MF_DAYS_BEFORE_NOW | NT_MF_HOURS_BEFORE_NOW | NT_MF_MINUTES_BEFORE_NOW => {
            let from = param(0, container)?;
            let dur = duration_till_now(&from)?;
            match op.op {
                NT_MF_DAYS_BEFORE_NOW => Ok(Variant::from_i64(dur.num_days())),
                NT_MF_HOURS_BEFORE_NOW => Ok(Variant::from_i64(dur.num_hours())),
                NT_MF_MINUTES_BEFORE_NOW => Ok(Variant::from_i64(dur.num_minutes())),
                _ => unreachable!(),
            }
        }
        NT_MF_DAYS_SINCE_WEEKDAY => {
            if op.params.len() != 1 {
                return Err(EvaluationError::new(
                    "Function `days_since_weekday` takes one parameter, the day of the week in numeric form",
                ));
            }
            let provided_day = param(0, container)?
                .as_numeric()
                .unwrap_or_default()
                .to_string()
                .parse::<i64>()
                .unwrap_or(0) as i32;
            if (1..=7).contains(&provided_day) {
                let today = (now_datetime().weekday().num_days_from_monday() as i32) + 1;
                let provided_day_last_week = 7 - (today - provided_day).abs();
                let result = if provided_day > today {
                    provided_day_last_week
                } else {
                    today - provided_day
                };
                Ok(Variant::from_i32(result))
            } else {
                Err(EvaluationError::new(
                    "Provided day of week was invalid, needs to be between 1 and 7.",
                ))
            }
        }

        NT_MF_SIZE => {
            let v = param(0, container)?;
            Ok(Variant::from_i32(v.size()))
        }
        NT_MF_IS_STRING => Ok(Variant::Boolean(param(0, container)?.is_string())),
        NT_MF_IS_NUMBER => Ok(Variant::Boolean(param(0, container)?.is_numeric())),
        NT_MF_IS_ARRAY => Ok(Variant::Boolean(param(0, container)?.is_array())),
        NT_MF_IS_NULL => Ok(Variant::Boolean(param(0, container)?.is_null())),
        NT_MF_TO_ARRAY => {
            let mut items = Vec::with_capacity(op.params.len());
            for p in &op.params {
                items.push(p.eval_inner(resolver, container, tracer)?);
            }
            Ok(Variant::Array(items))
        }
        NT_MF_IF => {
            if op.params.len() < 3 {
                return Err(EvaluationError::new(
                    "Function `if` takes 3 parameters. E.g. if(true, \"it is true\", \"it is false\")",
                ));
            }
            let cond = param(0, container)?;
            if cond.as_boolean() {
                param(1, container)
            } else {
                param(2, container)
            }
        }
        NT_MF_FIRST => {
            if op.params.len() != 2 {
                return Err(EvaluationError::new(format!(
                    "Function `{}` takes 2 parameters. E.g. {0}(\"Hello\", 2) will result in \"He\"",
                    function_name_from_code(op.op)
                )));
            }
            let s = param(0, container)?.as_string();
            let p1 = param(1, container)?
                .as_numeric()
                .unwrap_or_default()
                .to_string()
                .parse::<i64>()
                .unwrap_or(0) as i32;
            if p1 < 0 {
                Ok(Variant::empty_string())
            } else if (p1 as usize) >= s.len() {
                Ok(Variant::from_str(s))
            } else {
                Ok(Variant::from_str(&s[..p1 as usize]))
            }
        }
        NT_MF_LAST => {
            if op.params.len() != 2 {
                return Err(EvaluationError::new(format!(
                    "Function `{}` takes 2 parameters. E.g. {0}(\"Hello\", 2) will result in \"lo\"",
                    function_name_from_code(op.op)
                )));
            }
            let s = param(0, container)?.as_string();
            let p1 = param(1, container)?
                .as_numeric()
                .unwrap_or_default()
                .to_string()
                .parse::<i64>()
                .unwrap_or(0) as i32;
            let l = s.len() as i32;
            if p1 > l {
                Ok(Variant::from_str(s))
            } else if p1 < 0 {
                Ok(Variant::empty_string())
            } else {
                Ok(Variant::from_str(&s[(l - p1) as usize..]))
            }
        }
        NT_MF_SKIP => {
            if op.params.len() != 2 {
                return Err(EvaluationError::new(format!(
                    "Function {}` takes 2 parameters. E.g. {0}(\"Hello\", 2) will result in \"llo\"",
                    function_name_from_code(op.op)
                )));
            }
            let s = param(0, container)?.as_string();
            let p1 = param(1, container)?
                .as_numeric()
                .unwrap_or_default()
                .to_string()
                .parse::<i64>()
                .unwrap_or(0) as i32;
            let l = s.len() as i32;
            if p1 > l {
                Ok(Variant::empty_string())
            } else if p1 < 0 {
                Ok(Variant::from_str(s))
            } else {
                Ok(Variant::from_str(&s[p1 as usize..]))
            }
        }
        NT_MF_MAX => {
            let count = op.params.len();
            if count == 0 {
                return Ok(Variant::Null);
            }
            let first = param(0, container)?;
            let mut result = if first.is_array() {
                max_variant(&first.as_array())
            } else {
                to_numeric(first)
            };
            for i in 1..count {
                let v = param(i, container)?;
                let v = if v.is_array() {
                    max_variant(&v.as_array())
                } else {
                    to_numeric(v)
                };
                if v.compare(&result) == std::cmp::Ordering::Greater {
                    result = v;
                }
            }
            Ok(result)
        }
        NT_MF_MIN => {
            let count = op.params.len();
            if count == 0 {
                return Ok(Variant::Null);
            }
            let first = param(0, container)?;
            let mut result = if first.is_array() {
                min_variant(&first.as_array())
            } else {
                to_numeric(first)
            };
            for i in 1..count {
                let v = param(i, container)?;
                let v = if v.is_array() {
                    min_variant(&v.as_array())
                } else {
                    to_numeric(v)
                };
                if !v.is_null() && v.compare(&result) == std::cmp::Ordering::Less {
                    result = v;
                }
            }
            Ok(result)
        }
        NT_MF_AVERAGE => {
            let mut sum: f64 = 0.0;
            let mut count: i64 = 0;
            for p in &op.params {
                let v = p.eval_inner(resolver, container, tracer)?;
                if v.is_numeric() {
                    count += 1;
                    if let Some(d) = v.as_numeric() {
                        sum += d.to_string().parse::<f64>().unwrap_or(0.0);
                    }
                }
            }
            if count > 0 {
                Ok(Variant::from_f64(sum / count as f64))
            } else {
                Ok(Variant::Null)
            }
        }
        NT_MF_MEDIAN => {
            let mut values: Vec<rust_decimal::Decimal> = Vec::new();
            for p in &op.params {
                let v = p.eval_inner(resolver, container, tracer)?;
                if v.is_numeric() {
                    if let Some(d) = v.as_numeric() {
                        values.push(d);
                    }
                }
            }
            values.sort();
            if values.is_empty() {
                Ok(Variant::Null)
            } else if values.len() % 2 == 1 {
                Ok(Variant::Numeric(values[values.len() / 2]))
            } else {
                let a = values[values.len() / 2];
                let b = values[values.len() / 2 - 1];
                let mean = (a + b) / rust_decimal::Decimal::from(2u32);
                Ok(Variant::Numeric(mean.normalize()))
            }
        }
        NT_MF_FORMAT_TS => {
            if op.params.len() < 2 || op.params.len() > 3 {
                return Err(EvaluationError::new(format!(
                    "Function {}` takes 2 or 3 parameters. E.g. {0}(\"2023-02-28T05:16:55.835697363Z\", \"mm/dd/yy\") will result in \"02/28/23\"",
                    function_name_from_code(op.op)
                )));
            }
            let ts = param(0, container)?;
            if ts.is_null() {
                return Ok(Variant::Null);
            }
            let dt = if ts.is_numeric() {
                let ms = ts.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
                Utc.timestamp_millis_opt(ms).single().ok_or_else(|| {
                    EvaluationError::new(format!(
                        "Invalid timestamp. Function format_ts(ts, fmt, tz) takes timestamp parameter in String ISO format or numeric milliseconds from 1/1/1970. Provided ts: {}",
                        ts.as_string()
                    ))
                })?
            } else {
                parse_iso(&ts.as_string()).map_err(|_| {
                    EvaluationError::new(format!(
                        "Invalid timestamp. Function format_ts(ts, fmt, tz) takes timestamp parameter in String ISO format or numeric milliseconds from 1/1/1970. Provided ts: {}",
                        ts.as_string()
                    ))
                })?
            };
            let fmt = param(1, container)?.as_string();
            // Time zone (third optional parameter)
            if op.params.len() == 3 {
                let zone_id = param(2, container)?.as_string();
                let tz: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                    EvaluationError::new(format!(
                        "Invalid time zone ID. Function format_ts(ts, fmt, tz) takes tz as TimeZone ID. Provided time zone ID was: {}",
                        zone_id
                    ))
                })?;
                let local = dt.with_timezone(&tz);
                Ok(Variant::from_str(local.format(&fmt).to_string()))
            } else {
                Ok(Variant::from_str(dt.format(&fmt).to_string()))
            }
        }
        NT_FUNCTION => {
            let func = resolver
                .resolve_function(&op.name)
                .ok_or_else(|| EvaluationError::new(format!("Function \"{}\" is not defined", op.name)))?;
            // Evaluate all argument values against the caller container first
            // (mirrors Java where params run before binding into the local container).
            let param_names = func.parameter_names();
            let bound = param_names.len().min(op.params.len());
            let mut values: Vec<(String, Variant)> = Vec::with_capacity(bound);
            for idx in 0..bound {
                let param_name = &param_names[idx];
                if !param_name.is_empty() {
                    let value = op.params[idx].eval_inner(resolver, container, tracer)?;
                    values.push((param_name.clone(), value));
                }
            }
            let mut lvc = LocalVariantContainer::new(container);
            for (param_name, value) in values {
                lvc.set_variant(&param_name, value);
            }
            let mut result = func.execute(&mut lvc)?;
            if result.is_null() {
                if let Some(sub) = &op.substitution {
                    result = sub.eval_inner(resolver, container, tracer)?;
                }
            }
            Ok(result)
        }
        NT_MF_SWITCH => {
            if op.params.len() < 3 {
                return Err(EvaluationError::new("Function `switch` requires at least 3 parameters"));
            }
            let pivot = param(0, container)?;
            let mut result2 = Variant::Null;
            let mut i = 1;
            while i + 1 < op.params.len() {
                if op.params[i].eval_inner(resolver, container, tracer)?.value_equals(&pivot) {
                    result2 = op.params[i + 1].eval_inner(resolver, container, tracer)?;
                    break;
                }
                i += 2;
            }
            if result2.is_null() && op.params.len() % 2 == 0 {
                Ok(op.params[op.params.len() - 1].eval_inner(resolver, container, tracer)?)
            } else {
                Ok(result2)
            }
        }
        NT_MF_ROUND => {
            if op.params.len() != 2 {
                return Err(EvaluationError::new("Function `round` takes 2 parameters: value and decimal places."));
            }
            let value = param(0, container)?;
            let places = param(1, container)?
                .as_numeric()
                .unwrap_or_default()
                .to_string()
                .parse::<i64>()
                .unwrap_or(0) as i32;
            Variant::round(&value, places)
        }

        other => Err(EvaluationError::new(format!("Unexpected node: {}", other))),
    }
}

/// Assign ids in parse-like order (post-order: children first), mirroring the
/// Java parser's `idgen` counter for trace lookup.
fn assign_ids(node: &mut Node, next: &mut usize) {
    node.id = format!("n{}", *next);
    *next += 1;
    if let NodeKind::Op(o) = &mut node.kind {
        if let Some(l) = o.left.as_mut() {
            assign_ids(l, next);
        }
        if let Some(r) = o.right.as_mut() {
            assign_ids(r, next);
        }
        for p in o.params.iter_mut() {
            assign_ids(p, next);
        }
        if let Some(s) = o.substitution.as_mut() {
            assign_ids(s, next);
        }
    }
}

/// Format a trace message for an evaluated node (mirrors `TracingBaseNode`).
fn trace_message(op: &OpNode, result: &Variant, tracer: &Tracer) -> String {
    let func_name = function_name_from_code(op.op);
    let params_msg = op
        .params
        .iter()
        .map(|p| tracer.get(&p.id).to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if !func_name.is_empty() {
        return format!("FUN: {}({}) YIELDS {}", func_name, params_msg, result);
    }

    match op.op {
        NT_VARIABLE | NT_LOCAL_VARIABLE => {
            if params_msg.is_empty() {
                format!("GET: {} IS {}", op.name, result)
            } else {
                format!("GET: {}[{}] IS {}", op.name, params_msg, result)
            }
        }
        NT_FUNCTION => format!("FUN: {}({}) YIELDS {}", op.name, params_msg, result),
        op_code if op_code == '=' as i32 => {
            if params_msg.is_empty() {
                format!("SET: {} TO {}", op.name_for_tracing(), result)
            } else {
                format!("SET: {}[{}] TO {}", op.name_for_tracing(), params_msg, result)
            }
        }
        op_code if op_code == '!' as i32 => {
            let left_str = op
                .left
                .as_ref()
                .map(|n| tracer.get(&n.id).to_string())
                .unwrap_or_default();
            format!("OPR: NOT {} YIELDS {}", left_str, result)
        }
        op_code
            if [
                '+' as i32,
                '-' as i32,
                '*' as i32,
                '/' as i32,
                '&' as i32,
                '|' as i32,
                '>' as i32,
                '<' as i32,
                '^' as i32,
                NT_LOP_AND,
                NT_LOP_OR,
                NT_LOP_EQUALS,
                NT_LOP_NOT_EQUALS,
                NT_LOP_MORE_EQUALS,
                NT_LOP_LESS_EQUALS,
            ]
            .contains(&op_code) =>
        {
            let l = op
                .left
                .as_ref()
                .map(|n| tracer.get(&n.id).to_string())
                .unwrap_or_default();
            let r = op
                .right
                .as_ref()
                .map(|n| tracer.get(&n.id).to_string())
                .unwrap_or_default();
            format!(
                "OPR: {} {} {} YIELDS {}",
                l,
                op_to_string(op_code),
                r,
                result
            )
        }
        other => format!("OPR: {} YIELDS: {}", op_to_string(other), result),
    }
}

impl OpNode {
    /// In Java the trace of a `SET` node reads the name from the left node.
    fn name_for_tracing(&self) -> String {
        self.left
            .as_ref()
            .map(|n| match &n.kind {
                NodeKind::Op(o) => o.name.clone(),
                NodeKind::Const(_) => String::new(),
            })
            .unwrap_or_default()
    }
}

fn op_to_string(op: i32) -> String {
    match op {
        43 => "PLUS".to_string(),          // '+'
        45 => "MINUS".to_string(),         // '-'
        42 => "MULTIPLY".to_string(),      // '*'
        47 => "DIVIDE".to_string(),        // '/'
        38 => "LOGICAL AND".to_string(),   // '&'
        124 => "LOGICAL OR".to_string(),   // '|'
        62 => "MORE THAN".to_string(),     // '>'
        60 => "LESS THAN".to_string(),     // '<'
        94 => "POWER".to_string(),         // '^'
        NT_LOP_AND => "AND".to_string(),
        NT_LOP_OR => "OR".to_string(),
        NT_LOP_EQUALS => "EQUALS TO".to_string(),
        NT_LOP_NOT_EQUALS => "NOT EQUALS TO".to_string(),
        NT_LOP_MORE_EQUALS => "MORE OR EQUALS TO".to_string(),
        NT_LOP_LESS_EQUALS => "LESS OR EQUALS TO".to_string(),
        other => other.to_string(),
    }
}

fn assign_value(
    lhs: &Node,
    new_value: Variant,
    resolver: &dyn RunBlock,
    container: &mut dyn VariantContainer,
    tracer: Option<&RefCell<Tracer>>,
) -> VResult<Variant> {    match &lhs.kind {
        NodeKind::Const(_) => Err(EvaluationError::new("Invalid assignment")),
        NodeKind::Op(o) if o.op == NT_VARIABLE || o.op == NT_LOCAL_VARIABLE => {
            if !o.params.is_empty() {
                let index_v = o.params[0].eval_inner(resolver, container, tracer)?;
                let index = index_v
                    .as_numeric()
                    .unwrap_or_default()
                    .to_string()
                    .parse::<i64>()
                    .unwrap_or(0);
                if index >= 0 {
                    container.set_indexed_variant(&o.name, index as usize, new_value);
                }
            } else {
                container.set_variant(&o.name, new_value);
            }
            Ok(container.get_variant(&o.name))
        }
        _ => Err(EvaluationError::new("Invalid assignment")),
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn parse_iso(s: &str) -> Result<DateTime<Utc>, ()> {
    // Accept common ISO-8601 variants
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3f") {
        return Ok(Utc.from_utc_datetime(&naive));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&naive));
    }
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = naive.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&dt));
    }
    Err(())
}

fn parse_iso_to_millis(s: &str) -> VResult<i64> {
    parse_iso(s)
        .map(|dt| dt.timestamp_millis())
        .map_err(|_| EvaluationError::new(format!("Invalid ISO-8601 date: {}", s)))
}

#[allow(dead_code)]
fn parse_timestamp(v: &Variant) -> Result<DateTime<Utc>, ()> {
    if v.is_string() {
        parse_iso(&v.as_string())
    } else if v.is_numeric() {
        let ms = v.as_numeric().unwrap_or_default().to_string().parse::<i64>().map_err(|_| ())?;
        Utc.timestamp_millis_opt(ms).single().ok_or(())
    } else {
        Err(())
    }
}

fn duration_till_now(from: &Variant) -> VResult<chrono::Duration> {
    if from.is_string() {
        let start = parse_iso(&from.as_string())
            .map_err(|_| EvaluationError::new("ISO string date or millis is expected as input."))?;
        Ok(now_datetime().signed_duration_since(start))
    } else if from.is_numeric() {
        let ms = from.as_numeric().unwrap_or_default().to_string().parse::<i64>().unwrap_or(0);
        let start = Utc
            .timestamp_millis_opt(ms)
            .single()
            .ok_or_else(|| EvaluationError::new("ISO string date or millis is expected as input."))?;
        Ok(now_datetime().signed_duration_since(start))
    } else if from.is_null() {
        Err(EvaluationError::new("ISO string date or millis cannot not be null."))
    } else {
        Err(EvaluationError::new("ISO string date or millis is expected as input."))
    }
}

fn max_variant(va: &[Variant]) -> Variant {
    let mut max = Variant::Null;
    for vi in va {
        let vn = to_numeric(vi.clone());
        if (!vn.is_null() && vn.compare(&max) == std::cmp::Ordering::Greater) || max.is_null() {
            max = vn;
        }
    }
    max
}

fn min_variant(va: &[Variant]) -> Variant {
    let mut min = Variant::Null;
    for vi in va {
        let vn = to_numeric(vi.clone());
        if (!vn.is_null() && vn.compare(&min) == std::cmp::Ordering::Less) || min.is_null() {
            min = vn;
        }
    }
    min
}

fn to_numeric(v: Variant) -> Variant {
    if v.is_numeric() {
        v
    } else if v.is_null() {
        v
    } else {
        match v.as_numeric() {
            Some(d) => Variant::Numeric(d),
            None => Variant::Null,
        }
    }
}

// ---------------------------------------------------------------
// Parser (recursive descent, mirrors EquationParser.java)
// ---------------------------------------------------------------

/// Static function-name -> op-code table.
fn builtin_func_code(name: &str) -> i32 {
    use std::sync::OnceLock;
    static MAP: OnceLock<BTreeMap<&'static str, i32>> = OnceLock::new();
    let m = MAP.get_or_init(|| {
        BTreeMap::from([
            ("sin", NT_MF_SIN),
            ("cos", NT_MF_COS),
            ("asin", NT_MF_ASIN),
            ("acos", NT_MF_ACOS),
            ("tan", NT_MF_TAN),
            ("atan", NT_MF_ATAN),
            ("abs", NT_MF_ABS),
            ("neg", NT_MF_NEG),
            ("sqrt", NT_MF_SQRT),
            ("pow", NT_MF_POWER),
            ("log", NT_MF_LOG),
            ("exp", NT_MF_EXP),
            ("floor_mod", NT_MF_FLOOR_MOD),
            ("day", NT_MF_DAY),
            ("month", NT_MF_MONTH),
            ("year", NT_MF_YEAR),
            ("day_of_year", NT_MF_DAY_OF_YEAR),
            ("days_in_month", NT_MF_DAYS_IN_MONTH),
            ("now", NT_MF_NOW),
            ("iso", NT_MF_ISO),
            ("days_before_now", NT_MF_DAYS_BEFORE_NOW),
            ("hours_before_now", NT_MF_HOURS_BEFORE_NOW),
            ("minutes_before_now", NT_MF_MINUTES_BEFORE_NOW),
            ("days_since_weekday", NT_MF_DAYS_SINCE_WEEKDAY),
            ("day_of_week", NT_MF_DAY_OF_WEEK),
            ("format_ts", NT_MF_FORMAT_TS),
            ("size", NT_MF_SIZE),
            ("is_null", NT_MF_IS_NULL),
            ("is_numeric", NT_MF_IS_NUMBER),
            ("is_string", NT_MF_IS_STRING),
            ("is_array", NT_MF_IS_ARRAY),
            ("to_array", NT_MF_TO_ARRAY),
            ("if", NT_MF_IF),
            ("first", NT_MF_FIRST),
            ("skip", NT_MF_SKIP),
            ("last", NT_MF_LAST),
            ("max", NT_MF_MAX),
            ("min", NT_MF_MIN),
            ("avg", NT_MF_AVERAGE),
            ("median", NT_MF_MEDIAN),
            ("switch", NT_MF_SWITCH),
            ("round", NT_MF_ROUND),
        ])
    });
    let key = name.to_lowercase();
    m.get(key.as_str()).copied().unwrap_or(0)
}

/// Recursive-descent equation parser mirroring `EquationParser.java`.
pub struct EquationParser {
    source: Vec<char>,
    current_line: i32,
    position: usize,
    stop_at: usize,
}

impl EquationParser {
    pub fn new(source: &str) -> Self {
        let trimmed = source.trim().to_string();
        let source = if trimmed.is_empty() {
            configuration::expression_for_empty_eval()
        } else {
            trimmed
        };
        let chars: Vec<char> = source.chars().collect();
        let stop_at = chars.len();
        EquationParser {
            source: chars,
            current_line: 1,
            position: 0,
            stop_at,
        }
    }

    pub fn parse(&mut self, from: usize) -> VResult<Node> {
        self.parse_range(from, self.source.len())
    }

    pub fn parse_range(&mut self, from: usize, to: usize) -> VResult<Node> {
        self.stop_at = to.min(self.source.len());
        self.position = from;
        self.skip_spaces();
        let node = self.parse_assignment_node()?;
        self.stop_at = self.source.len();
        Ok(node)
    }

    /// Return any remaining unparsed source.
    pub fn unprocessed_source(&self) -> String {
        if self.position + 1 < self.stop_at {
            self.source[self.position..self.stop_at].iter().collect()
        } else {
            String::new()
        }
    }

    pub fn current_position(&self) -> usize {
        self.position
    }

    pub fn current_line(&self) -> i32 {
        self.current_line
    }

    // --- grammar ---

    fn parse_assignment_node(&mut self) -> VResult<Node> {
        let mut left = self.parse_logic_or_node()?;

        loop {
            self.skip_spaces();
            if self.current_char_check_sep() == '=' && self.char_at_check_sep(self.position + 1) != '='
            {
                if !left.is_variable_node() {
                    return Err(EvaluationError::coded(
                        CE_CONST_ASSIGNMENT,
                        self.current_line,
                        self.position as i32,
                    ));
                }
                self.forward_position();
                self.skip_spaces();
                let right = self.parse_logic_or_node()?;
                let mut node = Node::new_op('=' as i32);
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.left = Some(Box::new(left));
                    o.right = Some(Box::new(right));
                }
                left = node;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_logic_or_node(&mut self) -> VResult<Node> {
        let mut left = self.parse_logic_and_node()?;
        loop {
            self.skip_spaces();
            if self.current_char_check_sep() == '|'
                && self.char_at_check_sep(self.position + 1) == '|'
            {
                self.forward_position();
                self.forward_position();
                self.skip_spaces();
                let right = self.parse_logic_and_node()?;
                let mut node = Node::new_op(NT_LOP_OR);
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.left = Some(Box::new(left));
                    o.right = Some(Box::new(right));
                }
                left = node;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_logic_and_node(&mut self) -> VResult<Node> {
        let mut left = self.parse_comparison_operator()?;
        loop {
            self.skip_spaces();
            if self.current_char_check_sep() == '&'
                && self.char_at_check_sep(self.position + 1) == '&'
            {
                self.forward_position();
                self.forward_position();
                self.skip_spaces();
                let right = self.parse_comparison_operator()?;
                let mut node = Node::new_op(NT_LOP_AND);
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.left = Some(Box::new(left));
                    o.right = Some(Box::new(right));
                }
                left = node;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_plus_minus(&mut self) -> VResult<Node> {
        let mut left = self.parse_multiplication_division_operator()?;
        loop {
            self.skip_spaces();
            let c = self.current_char_check_sep();
            if c == '-' || c == '+' {
                self.forward_position();
                self.skip_spaces();
                let right = self.parse_multiplication_division_operator()?;
                let mut node = Node::new_op(c as i32);
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.left = Some(Box::new(left));
                    o.right = Some(Box::new(right));
                }
                left = node;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_multiplication_division_operator(&mut self) -> VResult<Node> {
        let mut left = self.parse_bin_and_or_operator()?;
        loop {
            self.skip_spaces();
            let c = self.current_char_check_sep();
            if c == '*' || c == '/' {
                self.forward_position();
                self.skip_spaces();
                let right = self.parse_bin_and_or_operator()?;
                let mut node = Node::new_op(c as i32);
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.left = Some(Box::new(left));
                    o.right = Some(Box::new(right));
                }
                left = node;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_comparison_operator(&mut self) -> VResult<Node> {
        let left = self.parse_plus_minus()?;
        self.skip_spaces();

        let c = self.current_char_check_sep();
        let next = self.char_at_check_sep(self.position + 1);

        if c == '>' || c == '<' || (c == '=' && next == '=') || (c == '!' && next == '=') {
            let mut op_code = c as i32;
            if next == '=' {
                match c {
                    '>' => {
                        op_code = NT_LOP_MORE_EQUALS;
                        self.forward_position();
                    }
                    '<' => {
                        op_code = NT_LOP_LESS_EQUALS;
                        self.forward_position();
                    }
                    '=' => {
                        op_code = NT_LOP_EQUALS;
                        self.forward_position();
                    }
                    '!' => {
                        op_code = NT_LOP_NOT_EQUALS;
                        self.forward_position();
                    }
                    _ => {}
                }
            }
            self.forward_position();
            self.skip_spaces();
            let right = self.parse_plus_minus()?;
            let mut node = Node::new_op(op_code);
            if let NodeKind::Op(ref mut o) = node.kind {
                o.left = Some(Box::new(left));
                o.right = Some(Box::new(right));
            }
            return Ok(node);
        }

        Ok(left)
    }

    fn parse_bin_and_or_operator(&mut self) -> VResult<Node> {
        let mut left = self.parse_not_operator()?;
        loop {
            self.skip_spaces();
            let c = self.current_char_check_sep();
            let next = self.char_at_check_sep(self.position + 1);
            if (c == '&' && next != '&') || (c == '|' && next != '|') || c == '^' {
                self.forward_position();
                self.skip_spaces();
                let right = self.parse_not_operator()?;
                let mut node = Node::new_op(c as i32);
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.left = Some(Box::new(left));
                    o.right = Some(Box::new(right));
                }
                left = node;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_not_operator(&mut self) -> VResult<Node> {
        self.skip_spaces();
        if self.current_char_check_sep() == '!' {
            self.forward_position();
            self.skip_spaces();
            let child = self.parse_not_operator()?;
            let mut node = Node::new_op('!' as i32);
            if let NodeKind::Op(ref mut o) = node.kind {
                o.left = Some(Box::new(child));
            }
            return Ok(node);
        }
        self.parse_eq_factor()
    }

    fn parse_eq_factor(&mut self) -> VResult<Node> {
        self.skip_spaces();

        if self.current_char_check_sep() == '{' {
            let mut node = Node::new_op(NT_MF_TO_ARRAY);
            self.parse_params_for(&mut node, '{', '}', ',')?;
            return Ok(node);
        }

        if self.current_char_check_sep() == '(' {
            self.forward_position();
            self.skip_spaces();
            let node = if self.current_char_check_sep() == ')' {
                Node::constant(Variant::Null)
            } else {
                let inner = self.parse_assignment_node()?;
                self.skip_spaces();
                if self.current_char_check_sep() != ')' {
                    return Err(EvaluationError::coded(
                        CE_MISSING_BRACKET,
                        self.current_line,
                        self.position as i32,
                    ));
                }
                inner
            };
            self.forward_position();
            return Ok(node);
        }
        self.parse_as_identifiable()
    }

    fn parse_as_identifiable(&mut self) -> VResult<Node> {
        self.skip_spaces();

        let c = self.current_char();

        if c == '-' {
            self.forward_position();
            self.skip_spaces();
            if self.is_digit() {
                let pos = self.position;
                let word = self.read_word();
                let value_str = format!("-{}", word);
                return Variant::from_decimal_str(&value_str)
                    .map(Node::constant)
                    .map_err(|e| EvaluationError::at(e.message, self.current_line, pos as i32));
            } else {
                return Err(EvaluationError::at(
                    format!("Expecting negative digit, however found '{}'", c),
                    self.current_line,
                    self.position as i32,
                ));
            }
        } else if self.is_digit() {
            let pos = self.position;
            let word = self.read_word();
            return Variant::from_decimal_str(&word)
                .map(Node::constant)
                .map_err(|e| EvaluationError::at(e.message, self.current_line, pos as i32));
        } else if self.is_text() {
            let text = self.read_text_sequence()?;
            return Ok(Node::constant(Variant::from_str(text)));
        } else if self.is_literal_char() {
            let word = self.read_word();
            if word.is_empty() {
                return Err(EvaluationError::coded(
                    CE_SYNTAX,
                    self.current_line,
                    self.position as i32,
                ));
            }
            self.skip_spaces();
            let is_function = self.current_char_check_sep() == '(';
            let is_array = self.current_char_check_sep() == '[';
            let func_code = builtin_func_code(&word);

            if is_function {
                // Function call
                let mut node = if func_code > 0 {
                    Node::new_op(func_code)
                } else {
                    Node::named_op(NT_FUNCTION, &word)
                };
                let count = self.parse_params_for(&mut node, '(', ')', ',')?;
                if count == 0 && func_code > 0 {
                    if let NodeKind::Op(ref mut o) = node.kind {
                        o.params.push(Node::constant(Variant::Null));
                    }
                }
                self.skip_spaces();
                if self.current_char() == '?' {
                    self.forward_position();
                    let sub = self.parse_not_operator()?;
                    if let NodeKind::Op(ref mut o) = node.kind {
                        o.substitution = Some(Box::new(sub));
                    }
                }
                return Ok(node);
            } else if is_array {
                let mut node = Node::named_op(NT_VARIABLE, &word);
                self.parse_params_for(&mut node, '[', ']', ',')?;
                if let NodeKind::Op(ref o) = node.kind {
                    if o.params.len() != 1 {
                        return Err(EvaluationError::coded(
                            CE_SYNTAX,
                            self.current_line,
                            self.position as i32,
                        ));
                    }
                }
                self.skip_spaces();
                if self.current_char() == '?' {
                    self.forward_position();
                    let sub = self.parse_not_operator()?;
                    if let NodeKind::Op(ref mut o) = node.kind {
                        o.substitution = Some(Box::new(sub));
                    }
                }
                return Ok(node);
            } else {
                // Identifier or constant
                if word == "var" {
                    self.skip_spaces();
                    if self.is_literal_char() {
                        let name = self.read_word();
                        if let Some(c) = check_constants(&name) {
                            return Ok(c);
                        } else {
                            return Ok(Node::named_op(NT_LOCAL_VARIABLE, &name));
                        }
                    } else {
                        return Err(EvaluationError::coded(
                            CE_ILLEGAL_VAR_NAME,
                            self.current_line,
                            self.position as i32,
                        ));
                    }
                }
                if let Some(c) = check_constants(&word) {
                    return Ok(c);
                }
                let mut node = Node::named_op(NT_VARIABLE, &word);
                self.skip_spaces();
                if self.current_char() == '?' {
                    self.forward_position();
                    let sub = self.parse_not_operator()?;
                    if let NodeKind::Op(ref mut o) = node.kind {
                        o.substitution = Some(Box::new(sub));
                    }
                }
                return Ok(node);
            }
        }

        Err(EvaluationError::coded(
            CE_SYNTAX,
            self.current_line,
            self.position as i32,
        ))
    }

    fn parse_params_for(
        &mut self,
        node: &mut Node,
        open: char,
        close: char,
        sep: char,
    ) -> VResult<usize> {
        let mut count = 0;
        self.skip_spaces();
        if self.current_char_check_sep() == open {
            self.forward_position();
            loop {
                self.skip_spaces();
                if self.current_char() == close {
                    self.forward_position();
                    self.skip_spaces();
                    break;
                }
                let param = self.parse_assignment_node()?;
                if let NodeKind::Op(ref mut o) = node.kind {
                    o.params.push(param);
                }
                count += 1;
                self.skip_spaces();
                let c = self.current_char();
                if c == close {
                    self.forward_position();
                    self.skip_spaces();
                    break;
                } else if c != sep {
                    return Err(EvaluationError::at(
                        format!("Expected ',' however got '{}'", c),
                        self.current_line,
                        self.position as i32,
                    ));
                }
                self.forward_position();
                self.skip_spaces();
            }
            Ok(count)
        } else {
            Err(EvaluationError::at(
                format!("Expected enclosed parameter(s). Missing open {}", open),
                self.current_line,
                self.position as i32,
            ))
        }
    }

    // --- character utilities ---

    fn char_at(&self, pos: usize) -> char {
        if pos < self.stop_at {
            self.source[pos]
        } else if pos == self.stop_at {
            ' '
        } else {
            '\0'
        }
    }

    fn char_at_check_sep(&self, pos: usize) -> char {
        let c = self.char_at(pos);
        if c == ';' || c == ',' {
            '\0'
        } else {
            c
        }
    }

    fn current_char(&self) -> char {
        self.char_at(self.position)
    }

    fn current_char_check_sep(&self) -> char {
        self.char_at_check_sep(self.position)
    }

    fn skip_spaces(&mut self) {
        loop {
            let c = self.char_at_check_sep(self.position);
            if c == '\0' || c > ' ' {
                break;
            }
            if c == '\n' {
                self.current_line += 1;
            }
            self.position += 1;
        }
    }

    fn forward_position(&mut self) {
        self.position += 1;
    }

    fn read_word(&mut self) -> String {
        let mut ret = String::new();
        self.skip_spaces();
        if self.is_digit() {
            while self.is_digit() {
                ret.push(self.current_char_check_sep());
                self.forward_position();
            }
        } else if self.is_literal_char() {
            while self.is_literal_char() || self.is_digit() {
                ret.push(self.current_char_check_sep());
                self.forward_position();
            }
        } else if self.is_operator_symbol() {
            while self.is_operator_symbol() {
                ret.push(self.current_char_check_sep());
                self.forward_position();
            }
        }
        ret
    }

    fn is_digit(&self) -> bool {
        let c = self.char_at_check_sep(self.position);
        ('0'..='9').contains(&c) || c == '.'
    }

    fn is_literal_char(&self) -> bool {
        let c = self.char_at_check_sep(self.position);
        c.is_ascii_uppercase() || c.is_ascii_lowercase() || c == '_'
    }

    fn is_text(&self) -> bool {
        self.char_at_check_sep(self.position) == '"'
    }

    fn is_operator_symbol(&self) -> bool {
        matches!(
            self.char_at_check_sep(self.position),
            '!' | '*' | '/' | '+' | '-' | '&' | '|' | '>' | '<' | '=' | '$' | '^'
        )
    }

    fn read_text_sequence(&mut self) -> VResult<String> {
        let mut text = String::new();
        if self.is_text() {
            self.forward_position();
            let mut special_char = false;
            loop {
                let ch = self.current_char();
                self.forward_position();
                if ch != '\0' && (ch as u32) < 32 && ch != '\r' && ch != '\n' {
                    return Err(EvaluationError::coded(
                        CE_ILLEGAL_SYMBOL,
                        self.current_line,
                        self.position as i32,
                    ));
                } else if ch == '\r' || ch == '\n' || ch == '\0' {
                    return Err(EvaluationError::coded(
                        CE_MISSING_QUOTATION,
                        self.current_line,
                        self.position as i32,
                    ));
                }
                let ch = if ch == '\\' {
                    let real = self.current_char();
                    self.forward_position();
                    special_char = true;
                    real
                } else {
                    ch
                };
                if special_char {
                    if ch == 'n' {
                        text.push('\r');
                        text.push('\n');
                    } else if ch == 't' {
                        text.push('\t');
                    } else {
                        text.push(ch);
                    }
                } else if ch == '"' {
                    break;
                } else {
                    text.push(ch);
                }
                special_char = false;
            }
        }
        Ok(text)
    }
}

fn check_constants(name: &str) -> Option<Node> {
    match name {
        "true" => Some(Node::constant(Variant::Boolean(true))),
        "false" => Some(Node::constant(Variant::Boolean(false))),
        "PI" => Some(Node::constant(Variant::from_f64(std::f64::consts::PI))),
        "E" => Some(Node::constant(Variant::from_f64(std::f64::consts::E))),
        "NULL" | "null" => Some(Node::constant(Variant::Null)),
        _ => None,
    }
}

// ---------------------------------------------------------------
// Top-level evaluation (EquationEval + CompositeNode)
// ---------------------------------------------------------------

/// Compiled vscript equation — mirror of `EquationEval`.
pub struct EquationEval {
    nodes: Vec<Node>,
    original_source: String,
}

impl EquationEval {
    /// Parse and compile an equation, mirroring the `EquationEval(String)` constructor.
    pub fn new(equation: &str) -> VResult<Self> {
        let mut parser = EquationParser::new(equation);
        let mut nodes = Vec::new();
        let mut pos = 0;
        loop {
            let node = parser.parse(pos)?;
            nodes.push(node);
            pos = parser.current_position() + 1;
            let leftover = parser.unprocessed_source();
            if leftover.len() > 1 && leftover.starts_with(';') {
                continue;
            }
            if !leftover.is_empty() {
                return Err(EvaluationError::at(
                    format!("Expression error: Unexpected text \"{}\"", leftover),
                    parser.current_line(),
                    parser.current_position() as i32,
                ));
            }
            break;
        }
        // Assign node ids (n1, n2, ...) for tracing, mirroring Java's idgen.
        let mut idgen = 1usize;
        for n in nodes.iter_mut() {
            assign_ids(n, &mut idgen);
        }
        Ok(EquationEval {
            nodes,
            original_source: equation.to_string(),
        })
    }

    /// Get collected reference stats.
    pub fn stats(&self) -> NodeStats {
        let mut stats = NodeStats::default();
        for n in &self.nodes {
            n.collect_stats(&mut stats);
        }
        stats
    }

    /// Evaluate with a function-resolving run block.
    pub fn eval_with(
        &self,
        resolver: &dyn RunBlock,
        container: &mut dyn VariantContainer,
    ) -> VResult<Variant> {
        let mut last = Variant::Null;
        for n in &self.nodes {
            last = n.eval(resolver, container)?;
        }
        Ok(last)
    }

    /// Evaluate with an attached trace listener (Java `EquationEval(.., traceListener)`).
    /// Per original semantics, an evaluation error on a traced node is reported
    /// via the listener and the node value becomes `null` instead of failing.
    pub fn eval_traced(
        &self,
        resolver: &dyn RunBlock,
        container: &mut dyn VariantContainer,
        listener: &dyn crate::tracer::TraceListener,
    ) -> VResult<Variant> {
        listener.trace(&format!("DSL: {}", self.original_source));
        let tracer = RefCell::new(Tracer::new(listener));
        let mut last = Variant::Null;
        for n in &self.nodes {
            last = n.eval_inner(resolver, container, Some(&tracer))?;
        }
        Ok(last)
    }

    /// Evaluate without user-functions (`eval()`/`eval(container)` in Java).
    pub fn eval(&self, container: &mut dyn VariantContainer) -> VResult<Variant> {
        self.eval_with(&crate::run_block::NoopRunBlock, container)
    }

    /// Parse an equation returning the AST.
    pub fn parse(equation: &str) -> VResult<Self> {
        EquationEval::new(equation)
    }
}
