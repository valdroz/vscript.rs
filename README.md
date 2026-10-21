# vscript (Rust)

`vscript-rs` is a **native Rust port** of [valdroz/vscript](https://github.com/valdroz/vscript),
a lightweight expression-evaluation language designed for embedding in applications. It parses
and evaluates arithmetic, comparison, logical, and assignment expressions over a small set of
types, with built-in math/string/date/array functions, custom function registration, JSON data
support, and full tracing.

The port preserves the original language surface, operator precedence, and decimal semantics,
with `rust_decimal` standing in for Java's `BigDecimal` and `serde_json` standing in for Gson.

## Features

- **Data types** — `Numeric` (arbitrary-precision decimal), `String`, `Boolean`, `Array`
  (mixed-type), and `Null`.
- **Rich operators** — arithmetic, comparison, logical (`and` / `or` / `not`), bitwise, and
  assignment, with correct precedence.
- **Built-in functions** — 50+ functions for math, string, date, array, and type-checking.
- **Variables & scoping** — dynamic variables with local-scope support.
- **Null safety** — the `?` substitution operator for safe defaults.
- **Custom functions** — register your own functions with named parameters.
- **JSON integration** — bind `Variant` values into JSON documents and drive data sets.
- **Configurable** — global decimal scale, rounding mode, and case sensitivity.
- **Tracing** — observe the evaluation step-by-step through a listener.

## Quick start

```rust
use vscript::{EquationEval, DefaultVariantContainer, VariantContainer};

// Basic evaluation
let mut container = DefaultVariantContainer::new();
let result = EquationEval::parse("3 + 5 * 2")
    .unwrap()
    .eval(&mut container)
    .unwrap();
assert_eq!(result.as_numeric().unwrap().to_string(), "13");

// Multiple statements with variables
let mut container = DefaultVariantContainer::new();
let result = EquationEval::parse("a = 10; b = 20; c = a + b; c > 25")
    .unwrap()
    .eval(&mut container)
    .unwrap();
assert!(result.as_boolean());
```

### Variables via a container

```rust
use vscript::{EquationEval, DefaultVariantContainer, Variant, VariantContainer};

let mut container = DefaultVariantContainer::new();
container.set_variant("price", Variant::from_decimal_str("5.2").unwrap());

let result = EquationEval::parse("price * 1.1")
    .unwrap()
    .eval(&mut container)
    .unwrap();
assert_eq!(result.as_numeric().unwrap().round_dp(2).to_string(), "5.72");
```

### Custom function

```rust
use vscript::{EquationEval, DefaultRunBlock, VariantContainer};

let mut block = DefaultRunBlock::new();
block.register_closure("custom_multiply(first, second)", |lvc| {
    let a = lvc.get_variant("first");
    let b = lvc.get_variant("second");
    a.multiply(&b)
})
.unwrap();

let mut container = DefaultVariantContainer::new();
let result = EquationEval::parse("2 + custom_multiply(3, 4)")
    .unwrap()
    .eval_with(&block, &mut container)
    .unwrap();
assert_eq!(result.as_numeric().unwrap().to_string(), "14");
```

### Configurable decimal scale

```rust
use vscript::{configuration, EquationEval, DefaultVariantContainer, VariantContainer};

configuration::set_decimal_scale(2);
let mut container = DefaultVariantContainer::new();
let result = EquationEval::parse("res = 3 + 10.1; res == 13.1")
    .unwrap()
    .eval(&mut container)
    .unwrap();
assert!(result.as_boolean());
```

## Operators

| Category    | Operators                                                        |
|-------------|------------------------------------------------------------------|
| Arithmetic  | `+` `-` `*` `/` `%` `^`                                           |
| Comparison  | `==` `!=` `<` `<=` `>` `>=`                                       |
| Logical     | `and` `or` `not`                                                 |
| Bitwise     | `&` `\|` `^` `~` (on numeric operands)                            |
| Assignment  | `=` (also `+=` `-=` `*=` `/=`)                                    |
| Null default| `expr ? fallback`                                                |

## Built-in functions

A selection — see the full reference in the [original documentation](https://github.com/valdroz/vscript/blob/master/FUNCTIONS.md):

- **Math**: `abs`, `ceil`, `floor`, `round`, `sqrt`, `pow`, `max`, `min`
- **String**: `length`, `substring`, `lower`, `upper`, `trim`, `replace`
- **Type checks**: `is_numeric`, `is_string`, `is_boolean`, `is_array`, `is_null`
- **Array**: element access, concatenation, length, index manipulation
- **Date**: year/month/day extraction and comparison

## Data types

| Type     | Example              | Description                          |
|----------|----------------------|--------------------------------------|
| Numeric  | `42`, `3.14`, `-7`   | High-precision decimal               |
| String   | `"Hello"`            | Text enclosed in double quotes       |
| Boolean  | `true`, `false`      | Logical values                       |
| Array    | `{1, 2, "three"}`    | Mixed-type collection                |
| Null     | `null`               | Uninitialized or missing value       |

## Project layout

```
vscript-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs              # public API (EquationEval, Variant, Configuration)
│   ├── configuration.rs    # global settings (decimal scale, rounding, case sensitivity)
│   ├── constants.rs        # built-in function definitions
│   ├── container.rs        # VariantContainer trait + DefaultVariantContainer
│   ├── error.rs            # EvaluationError
│   ├── json.rs             # JsonVariantContainer, JsonDataSetMaker, JsonUtils
│   ├── node.rs             # parser + AST nodes (operators, functions, arrays)
│   ├── run_block.rs        # RunBlock trait + DefaultRunBlock (custom functions)
│   ├── tracer.rs           # TraceListener + Tracer
│   └── variant.rs          # Variant enum with full operator semantics
└── tests/
    ├── array_eval.rs
    ├── decimal_and_script.rs
    ├── equation_eval.rs
    ├── json_integration.rs
    └── resources/          # JSON fixtures
```

## Building & testing

```sh
cargo build
cargo test
```

The test suite contains **85 tests** mirroring the original Java test
suites (`EquationEvalTests`, `ArrayEvalTests`, `DecimalEvalTests`,
`ScriptEvalTests`, and the JSON container tests).

## License

Apache-2.0 (same license as the original project).
