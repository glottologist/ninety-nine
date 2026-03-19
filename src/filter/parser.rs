use regex::RegexBuilder;

use crate::error::NinetyNineError;
use crate::filter::ast::{FilterExpr, Predicate};
use crate::filter::lexer::Token;
use crate::runner::binary::BinaryKind;

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    const fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(), NinetyNineError> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(NinetyNineError::FilterParse {
                message: format!("expected {expected:?}, found {tok:?}"),
            }),
            None => Err(NinetyNineError::FilterParse {
                message: format!("expected {expected:?}, found end of input"),
            }),
        }
    }

    fn parse_expr(&mut self) -> Result<FilterExpr, NinetyNineError> {
        let left = self.parse_unary()?;
        self.parse_binary(left)
    }

    fn parse_binary(&mut self, left: FilterExpr) -> Result<FilterExpr, NinetyNineError> {
        match self.peek() {
            Some(Token::And) => {
                self.advance();
                let right = self.parse_unary()?;
                let combined = FilterExpr::And(vec![left, right]);
                self.parse_binary(combined)
            }
            Some(Token::Or) => {
                self.advance();
                let right = self.parse_unary()?;
                let combined = FilterExpr::Or(vec![left, right]);
                self.parse_binary(combined)
            }
            _ => Ok(left),
        }
    }

    fn parse_unary(&mut self) -> Result<FilterExpr, NinetyNineError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(FilterExpr::Not(Box::new(expr)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<FilterExpr, NinetyNineError> {
        match self.peek() {
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_token(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::Ident(_)) => self.parse_predicate(),
            Some(tok) => Err(NinetyNineError::FilterParse {
                message: format!("unexpected token: {tok:?}"),
            }),
            None => Err(NinetyNineError::FilterParse {
                message: "unexpected end of input".to_string(),
            }),
        }
    }

    fn parse_predicate(&mut self) -> Result<FilterExpr, NinetyNineError> {
        let field = match self.advance() {
            Some(Token::Ident(s)) => s.clone(), // clone: extracting owned value from token
            _ => {
                return Err(NinetyNineError::FilterParse {
                    message: "expected identifier".to_string(),
                });
            }
        };

        if self.peek() == Some(&Token::LParen) {
            self.advance();
            let arg = match self.advance() {
                Some(Token::Ident(s)) => s.clone(), // clone: extracting owned value from token
                Some(tok) => {
                    return Err(NinetyNineError::FilterParse {
                        message: format!("expected argument, found {tok:?}"),
                    });
                }
                None => {
                    return Err(NinetyNineError::FilterParse {
                        message: "expected argument, found end of input".to_string(),
                    });
                }
            };
            self.expect_token(&Token::RParen)?;
            return make_call_predicate(&field, &arg);
        }

        match field.as_str() {
            "flaky" => Ok(FilterExpr::Predicate(Predicate::Flaky)),
            "quarantined" => Ok(FilterExpr::Predicate(Predicate::Quarantined)),
            "all" => Ok(FilterExpr::Predicate(Predicate::All)),
            _ => {
                let re = build_regex(&field)?;
                Ok(FilterExpr::Predicate(Predicate::Test(re)))
            }
        }
    }
}

fn make_call_predicate(func: &str, arg: &str) -> Result<FilterExpr, NinetyNineError> {
    match func {
        "test" => {
            let re = build_regex(arg)?;
            Ok(FilterExpr::Predicate(Predicate::Test(re)))
        }
        "package" => Ok(FilterExpr::Predicate(Predicate::Package(arg.to_string()))),
        "binary" => Ok(FilterExpr::Predicate(Predicate::Binary(arg.to_string()))),
        "kind" => {
            let kind = match arg {
                "lib" => BinaryKind::Lib,
                "bin" => BinaryKind::Bin,
                "test" => BinaryKind::Test,
                "example" => BinaryKind::Example,
                _ => {
                    return Err(NinetyNineError::FilterParse {
                        message: format!("unknown binary kind: {arg}"),
                    });
                }
            };
            Ok(FilterExpr::Predicate(Predicate::Kind(kind)))
        }
        _ => Err(NinetyNineError::FilterParse {
            message: format!("unknown function: {func}"),
        }),
    }
}

fn build_regex(pattern: &str) -> Result<regex::Regex, NinetyNineError> {
    RegexBuilder::new(pattern)
        .dfa_size_limit(256 * 1024)
        .build()
        .map_err(|e| NinetyNineError::FilterParse {
            message: format!("invalid regex '{pattern}': {e}"),
        })
}

/// Parses a token stream into a filter expression AST.
///
/// # Errors
///
/// Returns `FilterParse` if the tokens cannot be parsed into a valid expression.
pub fn parse(tokens: Vec<Token>) -> Result<FilterExpr, NinetyNineError> {
    if tokens.is_empty() {
        return Err(NinetyNineError::FilterParse {
            message: "empty filter expression".to_string(),
        });
    }

    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;

    if parser.pos < parser.tokens.len() {
        return Err(NinetyNineError::FilterParse {
            message: format!("unexpected trailing tokens at position {}", parser.pos),
        });
    }

    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::lexer::tokenize;
    use proptest::prelude::*;
    use rstest::rstest;

    fn parse_str(input: &str) -> Result<FilterExpr, NinetyNineError> {
        let tokens = tokenize(input)?;
        parse(tokens)
    }

    #[rstest]
    #[case("test(foo)", true)]
    #[case("flaky", true)]
    #[case("quarantined", true)]
    #[case("all", true)]
    #[case("!flaky", true)]
    #[case("!!flaky", true)]
    #[case("flaky & quarantined", true)]
    #[case("flaky | quarantined", true)]
    #[case("(flaky)", true)]
    #[case("test(x) & !quarantined", true)]
    #[case("package(my_crate)", true)]
    #[case("kind(test)", true)]
    #[case("kind(lib)", true)]
    #[case("my_test", true)]
    #[case("", false)]
    #[case("kind(invalid)", false)]
    #[case("unknown_func(arg)", false)]
    #[case("test([invalid)", false)]
    fn parses_expressions(#[case] input: &str, #[case] expected_ok: bool) {
        assert_eq!(parse_str(input).is_ok(), expected_ok, "input: {input}");
    }

    proptest! {
        #[test]
        fn parse_never_panics(input in ".*") {
            let _ = parse_str(&input);
        }
    }
}
