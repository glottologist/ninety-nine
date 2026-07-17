use crate::error::NinetyNineError;
use crate::filter;
use crate::filter::eval::{TestMetadata, eval};
use crate::runner::RunnerBackend;
use crate::runner::listing::TestCase;
use crate::storage::StorageBackend;

pub struct SelectOpts<'a> {
    pub filter_expr: Option<&'a str>,
    pub confidence: f64,
}

/// Discovers test cases and applies an optional filter expression.
///
/// Returns `Ok(None)` when no tests match (after printing a short message).
///
/// # Errors
///
/// Returns runner or filter errors from discovery / evaluation.
pub async fn discover_and_filter_tests(
    backend: &RunnerBackend,
    storage: &StorageBackend,
    opts: &SelectOpts<'_>,
) -> Result<Option<Vec<TestCase>>, NinetyNineError> {
    let mut tests = backend.discover_tests("").await?;

    if tests.is_empty() {
        println!("No tests found.");
        return Ok(None);
    }

    let eval_ctx = filter::build_eval_context(storage, opts.confidence).await?;

    if let Some(expr_str) = opts.filter_expr {
        let expr = filter::compile_filter(expr_str)?;
        tests.retain(|tc| {
            let meta = TestMetadata {
                name: &tc.name,
                package_name: &tc.package_name,
                binary_name: &tc.binary_name,
                kind: &tc.binary_kind,
            };
            eval(&expr, &meta, &eval_ctx)
        });
    }

    if tests.is_empty() {
        println!("No tests matching filter.");
        return Ok(None);
    }

    tracing::info!(count = tests.len(), "discovered tests");
    Ok(Some(tests))
}
