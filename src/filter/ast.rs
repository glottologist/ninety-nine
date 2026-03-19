use regex::Regex;

use crate::runner::binary::BinaryKind;

#[derive(Debug)]
pub enum FilterExpr {
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
    Not(Box<FilterExpr>),
    Predicate(Predicate),
}

#[derive(Debug)]
pub enum Predicate {
    Test(Regex),
    Package(String),
    Binary(String),
    Kind(BinaryKind),
    Flaky,
    Quarantined,
    All,
}
