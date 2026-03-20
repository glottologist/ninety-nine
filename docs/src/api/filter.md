# Filter DSL API

The filter module implements a small domain-specific language for selecting tests. It consists of four stages: tokenization, parsing, context building, and evaluation.

## Pipeline Overview

```
input string → tokenize() → parse() → FilterExpr (AST)
                                            ↓
test metadata + EvalContext → eval() → bool (match/no-match)
```

## Compilation

### `compile_filter`

```rust
pub fn compile_filter(input: &str) -> Result<FilterExpr, NinetyNineError>
```

Compiles a filter expression string into an AST. This is the primary entry point for the filter DSL.

**Errors:** Returns `NinetyNineError::FilterParse` if the input is syntactically invalid.

### `build_eval_context`

```rust
pub async fn build_eval_context(
    storage: &impl Storage,
    confidence: f64,
) -> Result<EvalContext, NinetyNineError>
```

Pre-loads data from storage needed to evaluate predicates like `flaky()` and `quarantined()`.

The context contains:
- **Flaky test set:** tests where `probability_flaky > 0.01` and `confidence >= threshold`
- **Quarantined test set:** all currently quarantined tests

## AST Types

### `FilterExpr`

The abstract syntax tree for filter expressions.

```rust
pub enum FilterExpr {
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
    Not(Box<FilterExpr>),
    Predicate(Predicate),
}
```

### `Predicate`

Leaf-level predicates that match against test metadata.

```rust
pub enum Predicate {
    Test(Regex),       // match test name against regex
    Package(String),   // match package name
    Binary(String),    // match binary name
    Kind(BinaryKind),  // match binary kind (lib, bin, test, example)
    Flaky,             // test is currently flaky
    Quarantined,       // test is currently quarantined
    All,               // matches everything
}
```

## Tokenizer

### `tokenize`

```rust
pub fn tokenize(input: &str) -> Result<Vec<Token>, NinetyNineError>
```

Splits input into tokens.

### `Token`

```rust
pub enum Token {
    Ident(String),
    LParen,
    RParen,
    And,
    Or,
    Not,
    Equals,
}
```

## Parser

### `parse`

```rust
pub fn parse(tokens: Vec<Token>) -> Result<FilterExpr, NinetyNineError>
```

Recursive descent parser with this precedence (lowest to highest):

1. **Or** (`|`) — binary, left-associative
2. **And** (`&`) — binary, left-associative
3. **Not** (`!`) — unary prefix
4. **Primary** — parenthesized expressions or predicate calls

**Predicate syntax:**

```
test(pattern)       — regex match on test name
package(name)       — exact match on package name
binary(name)        — exact match on binary name
kind(lib|bin|test|example) — match binary kind
flaky()             — test is flaky
quarantined()       — test is quarantined
all()               — match everything
```

## Evaluator

### `eval`

```rust
pub fn eval(
    expr: &FilterExpr,
    meta: &TestMetadata,
    ctx: &EvalContext,
) -> bool
```

Evaluates a compiled `FilterExpr` against a test's metadata using a pre-built evaluation context.

### `TestMetadata`

```rust
pub struct TestMetadata {
    pub name: String,
    pub package_name: String,
    pub binary_name: String,
    pub kind: BinaryKind,
}
```

Constructed from a `TestCase` at evaluation time.

### `EvalContext`

```rust
pub struct EvalContext {
    pub flaky_tests: HashSet<String>,
    pub quarantined_tests: HashSet<String>,
}
```

Pre-computed sets for O(1) predicate evaluation.

## Examples

```
# All flaky tests in the "auth" package
package(auth) & flaky()

# Everything except quarantined tests
!quarantined()

# Tests matching a pattern OR known flaky tests
test(.*timeout.*) | flaky()

# Complex composition
(package(api) | package(core)) & !quarantined() & flaky()
```

See the [Filter DSL Guide](../guides/filter-dsl.md) for usage examples from the CLI.
