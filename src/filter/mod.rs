pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;

use std::collections::HashSet;

use crate::error::NinetyNineError;
use crate::filter::ast::FilterExpr;
use crate::filter::eval::EvalContext;
use crate::storage::Storage;

/// Compiles a filter expression string into a `FilterExpr`.
///
/// # Errors
///
/// Returns `FilterParse` if the input cannot be tokenized or parsed.
pub fn compile_filter(input: &str) -> Result<FilterExpr, NinetyNineError> {
    let tokens = lexer::tokenize(input)?;
    parser::parse(tokens)
}

/// Builds an `EvalContext` from storage, pre-loading flaky and quarantined test sets.
///
/// # Errors
///
/// Returns a storage error if the queries fail.
pub async fn build_eval_context(
    storage: &impl Storage,
    confidence: f64,
) -> Result<EvalContext, NinetyNineError> {
    let scores = storage.get_all_scores().await?;
    let flaky_tests: HashSet<String> = scores
        .iter()
        .filter(|s| s.probability_flaky > 0.01 && s.confidence >= confidence)
        .map(|s| s.test_name.to_string())
        .collect();

    let quarantined_entries = storage.get_quarantined_tests().await?;
    let quarantined_tests: HashSet<String> = quarantined_entries
        .into_iter()
        .map(|e| e.test_name.into_inner())
        .collect();

    Ok(EvalContext {
        flaky_tests,
        quarantined_tests,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case("my_test", "Predicate(Test")]
    #[case("flaky & !quarantined | test(foo)", "Or(")]
    fn compile_filter_produces_expected_variant(#[case] input: &str, #[case] expected: &str) {
        let expr = compile_filter(input).unwrap();
        let variant = format!("{expr:?}");
        assert!(variant.starts_with(expected), "got: {variant}");
    }

    proptest! {
        #[test]
        fn compile_filter_never_panics(input in "[a-z_&|!() ]{0,60}") {
            let _ = compile_filter(&input);
        }
    }
}
