//! Core `Variant` value type, mirroring Java `Variant` abstract class and its
//! subclasses (numeric, string, boolean, null, array).

use crate::configuration;
use crate::constants::*;
use crate::error::{EvaluationError, VResult};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use std::cmp::Ordering;

/// A scripting value. Java used an abstract class with subclass overrides;
/// idiomatic Rust uses an enum, with the same behavioural contract.
#[derive(Debug, Clone)]
pub enum Variant {
    /// Numeric value. Java used BigDecimal; we use rust_decimal::Decimal.
    Numeric(Decimal),
    /// String value.
    String(String),
    /// Boolean value.
    Boolean(bool),
    /// Null value (singleton semantics in Java; represented as a unit variant).
    Null,
    /// Array value.
    Array(Vec<Variant>),
}

impl Default for Variant {
    fn default() -> Self {
        Variant::Null
    }
}

impl Variant {
    // ---- Constructors (mirroring static factory methods) ----

    pub fn from_decimal(value: Decimal) -> Variant {
        Variant::Numeric(value)
    }

    pub fn from_decimal_str(value: &str) -> VResult<Variant> {
        value
            .parse::<Decimal>()
            .map(Variant::Numeric)
            .map_err(|e| EvaluationError::new(format!("Invalid numeric value '{}': {}", value, e)))
    }

    pub fn from_f64(value: f64) -> Variant {
        Variant::Numeric(Decimal::from_f64_retain(value).unwrap_or(Decimal::ZERO))
    }

    pub fn from_i64(value: i64) -> Variant {
        Variant::Numeric(Decimal::from(value))
    }

    pub fn from_i32(value: i32) -> Variant {
        Variant::Numeric(Decimal::from(value))
    }

    pub fn from_str(value: impl Into<String>) -> Variant {
        Variant::String(value.into())
    }

    pub fn from_bool(value: bool) -> Variant {
        Variant::Boolean(value)
    }

    pub fn from_array(value: Vec<Variant>) -> Variant {
        Variant::Array(value)
    }

    pub fn empty_array() -> Variant {
        Variant::Array(Vec::new())
    }

    pub fn null() -> Variant {
        Variant::Null
    }

    pub fn empty_string() -> Variant {
        Variant::String(String::new())
    }

    /// Java `Variant.sanitize` — turns an Option<Variant> into a variant (null if None).
    pub fn sanitize(opt: Option<Variant>) -> Variant {
        opt.unwrap_or(Variant::Null)
    }

    /// `Variant.sanitize(variant, substitution)`.
    pub fn sanitize_or(opt: Option<Variant>, substitution: Variant) -> Variant {
        opt.unwrap_or(substitution)
    }

    /// `Variant.setArrayItem(variant, index, value)` static helper.
    pub fn set_array_item(variant: Variant, index: usize, value: Variant) -> Variant {
        match variant {
            Variant::Array(mut arr) => {
                while arr.len() < index + 1 {
                    arr.push(Variant::Null);
                }
                arr[index] = value;
                Variant::Array(arr)
            }
            other => {
                let mut arr = vec![other];
                while arr.len() < index + 1 {
                    arr.push(Variant::Null);
                }
                arr[index] = value;
                Variant::Array(arr)
            }
        }
    }

    /// `Variant.getArrayItem(variant, index)` static helper.
    pub fn get_array_item(variant: &Variant, index: usize) -> Variant {
        match variant {
            Variant::Array(arr) => arr.get(index).cloned().unwrap_or(Variant::Null),
            _ => Variant::Null,
        }
    }

    // ---- Type predicates ----

    pub fn is_null(&self) -> bool {
        matches!(self, Variant::Null)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Variant::String(_))
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Variant::Numeric(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Variant::Array(_))
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Variant::Boolean(_))
    }

    // ---- Coercion (as*) ----

    /// `asNumeric()` — panics were exceptions in Java; here returns Decimal (zero where Java returned 0).
    /// Where Java returned `Option<BigDecimal>` for conversions, we approximate with Option<Decimal>.
    pub fn as_numeric(&self) -> Option<Decimal> {
        match self {
            Variant::Numeric(d) => Some(*d),
            Variant::String(s) => {
                let parsed: Result<Decimal, _> = s.parse();
                match parsed {
                    Ok(d) => {
                        let scale = configuration::decimal_scale() as u32;
                        // Java setScale() pads zeros when widening and rounds when narrowing.
                        let adjusted = if d.scale() < scale {
                            let mut x = d;
                            x.rescale(scale);
                            x
                        } else {
                            d.round_dp_with_strategy(scale, configuration::rounding_mode())
                        };
                        Some(adjusted)
                    }
                    Err(_) => None, // Java logged a warning and returned null
                }
            }
            Variant::Boolean(b) => Some(if *b { Decimal::ONE } else { Decimal::ZERO }),
            Variant::Null => Some(Decimal::ZERO),
            Variant::Array(arr) => Some(
                Decimal::from(arr.len()).round_dp_with_strategy(
                    configuration::decimal_scale() as u32,
                    configuration::rounding_mode(),
                ),
            ),
        }
    }

    /// `asBoolean()`.
    pub fn as_boolean(&self) -> bool {
        match self {
            Variant::Numeric(d) => !d.is_zero(),
            Variant::String(s) => s.eq_ignore_ascii_case("true"),
            Variant::Boolean(b) => *b,
            Variant::Null => false,
            Variant::Array(arr) => !arr.is_empty(),
        }
    }

    /// `asString()`.
    pub fn as_string(&self) -> String {
        match self {
            Variant::Numeric(d) => d.to_string(),
            Variant::String(s) => s.clone(),
            Variant::Boolean(b) => b.to_string(),
            Variant::Null => String::new(),
            Variant::Array(arr) => {
                let mut ret = String::from("{");
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        ret.push(',');
                    }
                    ret.push_str(&v.as_string());
                }
                ret.push('}');
                ret
            }
        }
    }

    /// `asArray()` — default returns a list containing `self` for non-array variants.
    pub fn as_array(&self) -> Vec<Variant> {
        match self {
            Variant::Array(arr) => arr.clone(),
            other => vec![other.clone()],
        }
    }

    /// `size()`.
    pub fn size(&self) -> i32 {
        match self {
            Variant::String(s) => s.chars().count() as i32,
            Variant::Array(arr) => arr.len() as i32,
            _ => -1,
        }
    }

    // ---- Ordering (mirrors compareTo) ----

    pub fn compare(&self, other: &Variant) -> Ordering {
        match self {
            Variant::Numeric(a) => {
                let b = other.as_numeric().unwrap_or(Decimal::ZERO);
                a.cmp(&b)
            }
            Variant::String(a) => {
                let b = other.as_string();
                a.cmp(&b)
            }
            Variant::Boolean(a) => a.cmp(&other.as_boolean()),
            Variant::Null => {
                if other.is_null() {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            }
            Variant::Array(a) => {
                if let Variant::Array(b) = other {
                    a.len().cmp(&b.len())
                } else if a.is_empty() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
        }
    }

    // ---- Arithmetic operators ----

    fn invalid_operator(&self, operator: &str) -> EvaluationError {
        EvaluationError::new(format!("Invalid {} operator on [{}]", operator, self))
    }

    pub fn multiply(&self, other: &Variant) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => {
                let b = other.as_numeric().unwrap_or(Decimal::ZERO);
                Ok(Variant::Numeric(a * b))
            }
            Variant::String(s) => {
                if other.is_numeric() {
                    if let Ok(dec) = s.parse::<Decimal>() {
                        return Variant::Numeric(dec).multiply(other);
                    }
                }
                Err(self.invalid_operator("multiply"))
            }
            Variant::Boolean(_) | Variant::Array(_) => Err(self.invalid_operator("multiply")),
            Variant::Null => Ok(Variant::Null),
        }
    }

    pub fn add(&self, other: &Variant) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => {
                let b = other.as_numeric().unwrap_or(Decimal::ZERO);
                Ok(Variant::Numeric(a + b))
            }
            Variant::String(s) => {
                if other.is_numeric() {
                    if let Ok(dec) = s.parse::<Decimal>() {
                        return Variant::Numeric(dec).add(other);
                    }
                }
                Ok(Variant::String(format!("{}{}", s, other.as_string())))
            }
            Variant::Boolean(_) => Err(self.invalid_operator("add")),
            Variant::Array(arr) => {
                let mut new_value = arr.clone();
                match other {
                    Variant::Array(other_arr) => new_value.extend(other_arr.iter().cloned()),
                    other => new_value.push(other.clone()),
                }
                Ok(Variant::Array(new_value))
            }
            Variant::Null => Ok(Variant::Null),
        }
    }

    pub fn minus(&self, other: &Variant) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => {
                let b = other.as_numeric().unwrap_or(Decimal::ZERO);
                Ok(Variant::Numeric(a - b))
            }
            Variant::String(s) => {
                if other.is_numeric() {
                    if let Ok(dec) = s.parse::<Decimal>() {
                        return Variant::Numeric(dec).minus(other);
                    }
                }
                Err(self.invalid_operator("minus"))
            }
            Variant::Boolean(_) => Err(self.invalid_operator("minus")),
            Variant::Array(arr) => {
                let other_arr = other.as_array();
                let new_value: Vec<Variant> = arr
                    .iter()
                    .filter(|v| !other_arr.iter().any(|o| o == *v))
                    .cloned()
                    .collect();
                Ok(Variant::Array(new_value))
            }
            Variant::Null => Ok(Variant::Null),
        }
    }

    pub fn divide(&self, other: &Variant) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => {
                let scale = configuration::decimal_scale() as u32;
                let mode = configuration::rounding_mode();
                let divisor = other
                    .as_numeric()
                    .unwrap_or(Decimal::ZERO)
                    .round_dp_with_strategy(scale, mode);
                if divisor.is_zero() {
                    return Err(EvaluationError::new("Division by zero"));
                }
                let dividend = a.round_dp_with_strategy(scale, mode);
                // Java: divide(BigDecimal, RoundingMode) keeps the dividend's scale
                // for the quotient, then stripTrailingZeros().
                let quotient = (dividend / divisor).round_dp_with_strategy(scale, mode);
                Ok(Variant::Numeric(quotient.normalize()))
            }
            Variant::String(s) => {
                if other.is_numeric() {
                    if let Ok(dec) = s.parse::<Decimal>() {
                        return Variant::Numeric(dec).divide(other);
                    }
                }
                Err(self.invalid_operator("divide"))
            }
            Variant::Boolean(_) | Variant::Array(_) => Err(self.invalid_operator("divide")),
            Variant::Null => Ok(Variant::Null),
        }
    }

    pub fn pow(&self, other: &Variant) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => {
                let exp = other
                    .as_numeric()
                    .unwrap_or(Decimal::ZERO)
                    .to_i64()
                    .unwrap_or(0);
                // rust_decimal powi requires a non-negative u64
                if exp < 0 {
                    // fall back to f64 for negative exponents
                    let base = a.to_f64().unwrap_or(0.0);
                    return Ok(Variant::from_f64(base.powf(exp as f64)));
                }
                match a.checked_powi(exp) {
                    Some(d) => Ok(Variant::Numeric(d)),
                    None => {
                        let base = a.to_f64().unwrap_or(0.0);
                        Ok(Variant::from_f64(base.powf(exp as f64)))
                    }
                }
            }
            Variant::String(s) => {
                if other.is_numeric() {
                    if let Ok(dec) = s.parse::<Decimal>() {
                        return Variant::Numeric(dec).pow(other);
                    }
                }
                Err(self.invalid_operator("pow"))
            }
            Variant::Boolean(_) => Err(self.invalid_operator("pow")),
            Variant::Array(arr) => {
                let mut new_value = Vec::with_capacity(arr.len());
                for item in arr.iter() {
                    new_value.push(item.pow(other)?);
                }
                Ok(Variant::Array(new_value))
            }
            Variant::Null => Ok(Variant::Null),
        }
    }

    pub fn negate(&self) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => Ok(Variant::Numeric(-a)),
            Variant::Boolean(b) => Ok(Variant::Boolean(!b)),
            Variant::Null => Ok(Variant::Null),
            Variant::String(_) => Err(self.invalid_operator("negate")),
            Variant::Array(_) => Err(self.invalid_operator("negate")),
        }
    }

    /// `mfunc(func)` — apply a scalar built-in math function.
    pub fn mfunc(&self, func: i32) -> VResult<Variant> {
        match self {
            Variant::Numeric(a) => Self::mfunc_scalar(func, a.to_f64().unwrap_or(0.0)),
            Variant::String(s) => {
                let dec: Decimal = s.parse().map_err(|_| {
                    EvaluationError::new(format!(
                        "Invalid argument \"{}\" for function {}",
                        s,
                        function_name_from_code(func)
                    ))
                })?;
                Self::mfunc_scalar(func, dec.to_f64().unwrap_or(0.0))
            }
            Variant::Boolean(_) => Err(EvaluationError::new(format!(
                "Invalid argument {} for function {}",
                self,
                function_name_from_code(func)
            ))),
            Variant::Array(arr) => {
                let mut new_value = Vec::with_capacity(arr.len());
                for item in arr.iter() {
                    new_value.push(item.mfunc(func)?);
                }
                Ok(Variant::Array(new_value))
            }
            Variant::Null => Ok(Variant::Null),
        }
    }

    fn mfunc_scalar(func: i32, value: f64) -> VResult<Variant> {
        let result = match func {
            NT_MF_SIN => value.sin(),
            NT_MF_COS => value.cos(),
            NT_MF_ASIN => value.asin(),
            NT_MF_ACOS => value.acos(),
            NT_MF_TAN => value.tan(),
            NT_MF_ATAN => value.atan(),
            NT_MF_EXP => value.exp(),
            NT_MF_LN | NT_MF_LOG => value.ln(),
            NT_MF_SQRT => value.sqrt(),
            NT_MF_ABS => value.abs(),
            _ => return Ok(Variant::Null),
        };
        Ok(Variant::from_f64(result))
    }

    /// `Variant.round(value, decimalPlaces)`.
    pub fn round(value: &Variant, decimal_places: i32) -> VResult<Variant> {
        if decimal_places < 0 {
            return Err(EvaluationError::new(format!(
                "Invalid decimal places, must be a positive number: {}",
                decimal_places
            )));
        }

        let value = match value {
            Variant::Null => {
                if decimal_places == 0 {
                    Variant::from_i32(0)
                } else {
                    Variant::Numeric(Decimal::ZERO)
                }
            }
            other => other.clone(),
        };

        match value.as_numeric() {
            Some(num) => Ok(Variant::Numeric(num.round_dp_with_strategy(
                decimal_places as u32,
                // Java used RoundingMode.HALF_UP fixed here (not configurable)
                RoundingStrategy::MidpointAwayFromZero,
            ))),
            None => Ok(Variant::Null),
        }
    }

    // ---- Equality used by ==, !=, contains etc. ----

    pub fn value_equals(&self, other: &Variant) -> bool {
        match self {
            Variant::Numeric(a) => {
                if other.is_null() || other.is_array() {
                    return false;
                }
                let b = other.as_numeric().unwrap_or(Decimal::ZERO);
                *a == b
            }
            Variant::String(a) => {
                let case_sensitive = configuration::is_case_sensitive();
                match other {
                    Variant::Array(arr) => {
                        if case_sensitive {
                            arr.iter().any(|v| {
                                v.is_string() && v.as_string() == *a
                            })
                        } else {
                            arr.iter()
                                .any(|v| a.eq_ignore_ascii_case(&v.as_string()))
                        }
                    }
                    other => {
                        if case_sensitive {
                            *a == other.as_string()
                        } else {
                            a.eq_ignore_ascii_case(&other.as_string())
                        }
                    }
                }
            }
            Variant::Boolean(a) => *a == other.as_boolean(),
            Variant::Null => other.is_null(),
            Variant::Array(arr) => match other {
                Variant::Array(other_arr) => {
                    // Java: containsAll — self contains all of other's items
                    other_arr.iter().all(|o| arr.iter().any(|s| s.value_equals(o)))
                }
                other => arr.iter().any(|s| s.value_equals(other)),
            },
        }
    }
}

/// Mirrors Java `EquationParser.functionNameFromCode`.
pub fn function_name_from_code(code: i32) -> &'static str {
    use std::collections::HashMap;
    use std::sync::OnceLock;

    static MAP: OnceLock<HashMap<i32, &'static str>> = OnceLock::new();
    let m = MAP.get_or_init(|| {
        let pairs: Vec<(&'static str, i32)> = vec![
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
        ];
        pairs.into_iter().map(|(k, v)| (v, k)).collect()
    });
    m.get(&code).copied().unwrap_or("")
}

impl std::fmt::Display for Variant {
    /// Mirrors Java `toString()` on each subclass:
    /// - numeric: plaintext decimal
    /// - string: "quoted"
    /// - boolean: true/false
    /// - null: "null"
    /// - array: [a, b, ...]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variant::Numeric(d) => write!(f, "{}", d),
            Variant::String(s) => write!(f, "\"{}\"", s),
            Variant::Boolean(b) => write!(f, "{}", b),
            Variant::Null => write!(f, "null"),
            Variant::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl PartialEq for Variant {
    fn eq(&self, other: &Self) -> bool {
        self.value_equals(other)
    }
}
