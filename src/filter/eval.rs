use std::collections::HashSet;

use crate::filter::ast::{FilterExpr, Predicate};
use crate::runner::binary::BinaryKind;

pub struct TestMetadata<'a> {
    pub name: &'a str,
    pub package_name: &'a str,
    pub binary_name: &'a str,
    pub kind: &'a BinaryKind,
}

pub struct EvalContext {
    pub flaky_tests: HashSet<String>,
    pub quarantined_tests: HashSet<String>,
}

/// Evaluates a filter expression against test metadata.
#[must_use]
pub fn eval(expr: &FilterExpr, test: &TestMetadata<'_>, ctx: &EvalContext) -> bool {
    match expr {
        FilterExpr::And(items) => items.iter().all(|e| eval(e, test, ctx)),
        FilterExpr::Or(items) => items.iter().any(|e| eval(e, test, ctx)),
        FilterExpr::Not(inner) => !eval(inner, test, ctx),
        FilterExpr::Predicate(pred) => eval_predicate(pred, test, ctx),
    }
}

fn eval_predicate(pred: &Predicate, test: &TestMetadata<'_>, ctx: &EvalContext) -> bool {
    match pred {
        Predicate::Test(regex) => regex.is_match(test.name),
        Predicate::Package(name) => test.package_name.contains(name.as_str()),
        Predicate::Binary(name) => test.binary_name.contains(name.as_str()),
        Predicate::Kind(kind) => test.kind == kind,
        Predicate::Flaky => ctx.flaky_tests.contains(test.name),
        Predicate::Quarantined => ctx.quarantined_tests.contains(test.name),
        Predicate::All => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::lexer::tokenize;
    use crate::filter::parser::parse;
    use proptest::prelude::*;
    use rstest::rstest;

    fn make_ctx() -> EvalContext {
        let mut flaky = HashSet::new();
        flaky.insert("tests::flaky_one".to_string());
        let mut quarantined = HashSet::new();
        quarantined.insert("tests::quarantined_one".to_string());
        EvalContext {
            flaky_tests: flaky,
            quarantined_tests: quarantined,
        }
    }

    fn make_meta(name: &str) -> TestMetadata<'_> {
        TestMetadata {
            name,
            package_name: "my-crate",
            binary_name: "my-crate-test",
            kind: &BinaryKind::Test,
        }
    }

    #[rstest]
    #[case("flaky", "tests::flaky_one", true)]
    #[case("flaky", "tests::stable_one", false)]
    #[case("quarantined", "tests::quarantined_one", true)]
    #[case("quarantined", "tests::stable_one", false)]
    #[case("all", "anything", true)]
    #[case("test(flaky)", "tests::flaky_one", true)]
    #[case("test(flaky)", "tests::stable_one", false)]
    #[case("!flaky", "tests::stable_one", true)]
    #[case("!flaky", "tests::flaky_one", false)]
    fn eval_filter_expressions(
        #[case] filter: &str,
        #[case] test_name: &str,
        #[case] expected: bool,
    ) {
        let tokens = tokenize(filter).unwrap();
        let expr = parse(tokens).unwrap();
        let ctx = make_ctx();
        let meta = make_meta(test_name);
        assert_eq!(eval(&expr, &meta, &ctx), expected);
    }

    proptest! {
        #[test]
        fn double_negation_is_identity(name in "[a-z_]{1,20}") {
            let tokens = tokenize(&format!("!!test({name})")).unwrap();
            let expr = parse(tokens).unwrap();
            let single_tokens = tokenize(&format!("test({name})")).unwrap();
            let single_expr = parse(single_tokens).unwrap();
            let ctx = make_ctx();
            let meta = make_meta(&name);
            prop_assert_eq!(eval(&expr, &meta, &ctx), eval(&single_expr, &meta, &ctx));
        }
    }
}
