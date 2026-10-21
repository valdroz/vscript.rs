//! Error type mirroring Java `EvaluationException`.

use crate::constants;
use std::fmt;

/// Evaluation error equivalent to the Java `EvaluationException` (runtime).
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationError {
    pub message: String,
}

impl EvaluationError {
    pub fn new(message: impl Into<String>) -> Self {
        EvaluationError {
            message: message.into(),
        }
    }

    /// Mirrors `EvaluationException(String message, int lineNumber, int pos)`.
    pub fn at(message: impl Into<String>, line_number: i32, pos: i32) -> Self {
        EvaluationError {
            message: format!(
                "Error: {}; Line {}, Pos {}",
                message.into(),
                line_number,
                pos
            ),
        }
    }

    /// Mirrors `EvaluationException(int code, int lineNumber, int pos)`.
    pub fn coded(code: i32, line_number: i32, pos: i32) -> Self {
        EvaluationError {
            message: format!(
                "Error ({}): {}; Line: {}, Pos: {}",
                code,
                error_code_to_message(code),
                line_number,
                pos
            ),
        }
    }
}

fn error_code_to_message(code: i32) -> &'static str {
    match code {
        constants::CE_ILLEGAL_SYMBOL => "Illegal symbol",
        constants::CE_INCOMPLETE => "Incomplete statement",
        constants::CE_MISSING_BRACKET => "Missing ')' bracket",
        constants::CE_MISSING_BRACKET2 => "Missing '}' bracket",
        constants::CE_MISSING_BRACKET3 => "Missing ']' bracket",
        constants::CE_MISSING_QUOTATION => "Missing quotation",
        constants::CE_RESERVED_WORD_AS_FUNC => "Reserved keyword used",
        constants::CE_SYNTAX => "Syntax error",
        constants::CE_TEXT_SYNTAX => "String definition error",
        constants::CE_UNSUPPORTED_FUNCTION => "Unsupported function name",
        constants::CE_UNSUPPORTED_STATEMENT => "Unsupported statement or function name",
        constants::CE_FUNC_DECL_SYNTAX => "Function declaration syntax error",
        constants::CE_MISSING_COMMA => "Missing comma",
        constants::CE_ILLEGAL_VAR_NAME => "Illegal variable name",
        constants::CE_CONST_ASSIGNMENT => "Constant assigned is not allowed",
        _ => "Unknown error",
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EvaluationError {}

/// Convenience alias used throughout the crate.
pub type VResult<T> = Result<T, EvaluationError>;
