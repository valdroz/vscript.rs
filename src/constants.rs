//! Internal constants mirroring the Java `Constants` interface.

// Error codes
pub const CE_INCOMPLETE: i32 = 1;
pub const CE_SYNTAX: i32 = 2;
pub const CE_RESERVED_WORD_AS_FUNC: i32 = 3;
pub const CE_MISSING_BRACKET: i32 = 4; // )
pub const CE_TEXT_SYNTAX: i32 = 5;
pub const CE_UNSUPPORTED_FUNCTION: i32 = 6;
pub const CE_ILLEGAL_SYMBOL: i32 = 7;
pub const CE_MISSING_QUOTATION: i32 = 8;
pub const CE_MISSING_BRACKET2: i32 = 9; // }
pub const CE_UNSUPPORTED_STATEMENT: i32 = 10;
pub const CE_FUNC_DECL_SYNTAX: i32 = 11;
pub const CE_MISSING_COMMA: i32 = 12;
pub const CE_ILLEGAL_VAR_NAME: i32 = 13;
pub const CE_MISSING_BRACKET3: i32 = 14; // ]
pub const CE_CONST_ASSIGNMENT: i32 = 15;

// Node types
pub const NT_VARIABLE: i32 = 134;
pub const NT_FUNCTION: i32 = 135;
pub const NT_LOCAL_VARIABLE: i32 = 137;

// Built-in math/script function codes
pub const NT_MF_SIN: i32 = 140;
pub const NT_MF_COS: i32 = 141;
pub const NT_MF_ASIN: i32 = 142;
pub const NT_MF_ACOS: i32 = 143;
pub const NT_MF_TAN: i32 = 144;
pub const NT_MF_ATAN: i32 = 145;
pub const NT_MF_ABS: i32 = 146;
pub const NT_MF_NEG: i32 = 147;
pub const NT_MF_SQRT: i32 = 148;
pub const NT_MF_LN: i32 = 149;
pub const NT_MF_LOG: i32 = 150;
pub const NT_MF_EXP: i32 = 151;
pub const NT_MF_DAY: i32 = 153;
pub const NT_MF_MONTH: i32 = 154;
pub const NT_MF_YEAR: i32 = 155;
pub const NT_MF_DAY_OF_YEAR: i32 = 156;
pub const NT_MF_DAYS_IN_MONTH: i32 = 157;
pub const NT_MF_SIZE: i32 = 158;
pub const NT_MF_IS_STRING: i32 = 159;
pub const NT_MF_IS_NUMBER: i32 = 160;
pub const NT_MF_IS_ARRAY: i32 = 161;
pub const NT_MF_IS_NULL: i32 = 162;
pub const NT_MF_NOW: i32 = 163;
pub const NT_MF_ISO: i32 = 164;
pub const NT_MF_HOURS_BEFORE_NOW: i32 = 165;
pub const NT_MF_DAYS_BEFORE_NOW: i32 = 166;
pub const NT_MF_TO_ARRAY: i32 = 167;
pub const NT_MF_MINUTES_BEFORE_NOW: i32 = 168;
pub const NT_MF_POWER: i32 = 169;
pub const NT_MF_IF: i32 = 170;
pub const NT_MF_FIRST: i32 = 171;
pub const NT_MF_SKIP: i32 = 172;
pub const NT_MF_LAST: i32 = 173;
pub const NT_MF_MAX: i32 = 174;
pub const NT_MF_MIN: i32 = 175;
pub const NT_MF_MEDIAN: i32 = 176;
pub const NT_MF_AVERAGE: i32 = 177;
pub const NT_MF_DAYS_SINCE_WEEKDAY: i32 = 178;
pub const NT_MF_FLOOR_MOD: i32 = 179;
pub const NT_MF_FORMAT_TS: i32 = 180;
pub const NT_MF_SWITCH: i32 = 181;
pub const NT_MF_ROUND: i32 = 182;
pub const NT_MF_DAY_OF_WEEK: i32 = 183;

// Logical operators
pub const NT_LOP_AND: i32 = 128;
pub const NT_LOP_OR: i32 = 129;
pub const NT_LOP_MORE_EQUALS: i32 = 130;
pub const NT_LOP_LESS_EQUALS: i32 = 131;
pub const NT_LOP_EQUALS: i32 = 132;
pub const NT_LOP_NOT_EQUALS: i32 = 136;
