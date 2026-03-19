use crate::error::NinetyNineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Ident(String),
}

/// Tokenizes a filter expression string.
///
/// # Errors
///
/// Returns `FilterParse` for unexpected characters.
pub fn tokenize(input: &str) -> Result<Vec<Token>, NinetyNineError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '&' => {
                chars.next();
                tokens.push(Token::And);
            }
            '|' => {
                chars.next();
                tokens.push(Token::Or);
            }
            '!' => {
                chars.next();
                tokens.push(Token::Not);
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '(' || c == ')' || c == '&' || c == '|' || c == '!' || c.is_whitespace()
                    {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                if word.is_empty() {
                    return Err(NinetyNineError::FilterParse {
                        message: format!("unexpected character: {ch}"),
                    });
                }
                tokens.push(Token::Ident(word));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case("test(foo)", vec![Token::Ident("test".into()), Token::LParen, Token::Ident("foo".into()), Token::RParen])]
    #[case("flaky & !quarantined", vec![Token::Ident("flaky".into()), Token::And, Token::Not, Token::Ident("quarantined".into())])]
    #[case("a | b", vec![Token::Ident("a".into()), Token::Or, Token::Ident("b".into())])]
    fn tokenizes_correctly(#[case] input: &str, #[case] expected: Vec<Token>) {
        assert_eq!(tokenize(input).unwrap(), expected);
    }

    proptest! {
        #[test]
        fn tokenize_never_panics(input in "[ a-zA-Z0-9_():&|!]{0,100}") {
            let _ = tokenize(&input);
        }
    }
}
