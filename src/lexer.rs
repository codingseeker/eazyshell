use crate::error::ShellError;
use crate::token::{Token, TokenType};

const INIT_TOKEN_CAP: usize = 32;

fn is_operator_char(c: char) -> bool {
    matches!(c, '|' | '&' | ';' | '<' | '>' | '(' | ')')
}

fn word_continues_at(src: &[char], idx: usize) -> bool {
    if idx >= src.len() {
        return false;
    }
    let c = src[idx];
    !(c == ' ' || c == '\t' || is_operator_char(c))
}

fn is_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_identifier_cont(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn read_variable_name(src: &[char], start: usize) -> Option<(String, usize)> {
    if start >= src.len() {
        return None;
    }
    let c = src[start];
    if c.is_ascii_digit() {
        return Some((c.to_string(), start + 1));
    }
    if is_identifier_start(c) {
        let mut end = start + 1;
        while end < src.len() && is_identifier_cont(src[end]) {
            end += 1;
        }
        return Some((src[start..end].iter().collect(), end));
    }
    if matches!(c, '?' | '$' | '!' | '*' | '@') {
        return Some((c.to_string(), start + 1));
    }
    None
}

fn read_braced_variable(src: &[char], start: usize) -> Result<(String, usize), ShellError> {
    let mut depth = 1usize;
    let mut i = start;
    while i < src.len() {
        match src[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let text: String = src[start..i].iter().collect();
                    return Ok((text, i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err(ShellError::lex("unclosed ${", start + 1))
}

fn read_word(src: &[char], pos: usize) -> Result<(String, bool, usize), ShellError> {
    let mut in_squote = false;
    let mut in_dquote = false;
    let mut escaped = false;
    let mut quoted = false;
    let mut i = pos;
    let mut col = pos + 1;

    while i < src.len() {
        let c = src[i];
        if escaped {
            escaped = false;
            i += 1;
            col += 1;
            continue;
        }
        if c == '\\' && !in_squote {
            escaped = true;
            quoted = true;
            i += 1;
            col += 1;
            continue;
        }
        if c == '\'' && !in_dquote {
            in_squote = !in_squote;
            quoted = true;
            i += 1;
            col += 1;
            continue;
        }
        if c == '"' && !in_squote {
            in_dquote = !in_dquote;
            quoted = true;
            i += 1;
            col += 1;
            continue;
        }
        if !in_squote && !in_dquote && (is_operator_char(c) || c == ' ' || c == '\t') {
            break;
        }
        if !in_squote && c == '$' && i + 1 < src.len() && src[i + 1] == '(' {
            let mut depth = 0usize;
            let mut j = i + 1;
            while j < src.len() {
                match src[j] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            i = j;
            col = i + 1;
            continue;
        }
        i += 1;
        col += 1;
    }

    if in_squote || in_dquote {
        return Err(ShellError::lex("unclosed quote", col));
    }
    if escaped {
        return Err(ShellError::lex("trailing escape", col));
    }

    let raw: String = src[pos..i].iter().collect();
    Ok((raw, quoted, i))
}

fn push_op(tokens: &mut Vec<Token>, type_: TokenType) {
    tokens.push(Token {
        type_,
        value: None,
        quoted: false,
    });
}

fn push_value(tokens: &mut Vec<Token>, type_: TokenType, value: String) {
    tokens.push(Token {
        type_,
        value: Some(value),
        quoted: false,
    });
}

pub fn tokenize(line: &str) -> Result<Vec<Token>, ShellError> {
    let src: Vec<char> = line.chars().collect();
    let len = src.len();
    let mut tokens: Vec<Token> = Vec::with_capacity(INIT_TOKEN_CAP);
    let mut pos = 0usize;

    while pos < len {
        let c = src[pos];

        if c == ' ' || c == '\t' {
            pos += 1;
            continue;
        }

        if c == '#' && (pos == 0 || src[pos - 1] == ' ' || src[pos - 1] == '\t') {
            break;
        }

        let next = src.get(pos + 1).copied();

        if c == '|' && next == Some('|') {
            push_op(&mut tokens, TokenType::Or);
            pos += 2;
            continue;
        }
        if c == '|' {
            push_op(&mut tokens, TokenType::Pipe);
            pos += 1;
            continue;
        }
        if c == '&' && next == Some('&') {
            push_op(&mut tokens, TokenType::And);
            pos += 2;
            continue;
        }
        if c == ';' {
            push_op(&mut tokens, TokenType::Semicolon);
            pos += 1;
            continue;
        }
        if c == '&' && next == Some('>') {
            push_op(&mut tokens, TokenType::RedirectBothOut);
            pos += 2;
            continue;
        }
        if c == '>' && next == Some('>') {
            push_op(&mut tokens, TokenType::RedirectAppend);
            pos += 2;
            continue;
        }
        if c == '>' && next == Some('|') {
            push_op(&mut tokens, TokenType::RedirectClobber);
            pos += 2;
            continue;
        }
        if c == '<' && next == Some('<') && src.get(pos + 2) == Some(&'-') {
            push_op(&mut tokens, TokenType::RedirectHeredocTab);
            pos += 3;
            continue;
        }
        if c == '<' && next == Some('<') {
            push_op(&mut tokens, TokenType::RedirectHeredoc);
            pos += 2;
            continue;
        }
        if c == '<' {
            push_op(&mut tokens, TokenType::RedirectIn);
            pos += 1;
            continue;
        }
        if c == '>' {
            push_op(&mut tokens, TokenType::RedirectOut);
            pos += 1;
            continue;
        }
        if c == '&' && next != Some('&') {
            push_op(&mut tokens, TokenType::Background);
            pos += 1;
            continue;
        }

        if c == '$' && next == Some('(') && src.get(pos + 2) == Some(&'(') {
            let mut i = pos + 3;
            let start = i;
            let mut paren_depth = 1usize;
            let mut col = pos + 3;
            while i < len {
                match src[i] {
                    '(' => paren_depth += 1,
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
                col += 1;
            }
            if paren_depth != 0 {
                return Err(ShellError::lex("unfinished $(( expression", col));
            }
            if word_continues_at(&src, i + 1) {
                let (word, quoted, after) = read_word(&src, pos)?;
                push_value(
                    &mut tokens,
                    if is_assignment_word(&word) {
                        TokenType::Assign
                    } else {
                        TokenType::Word
                    },
                    word,
                );
                tokens.last_mut().unwrap().quoted = quoted;
                pos = after;
                continue;
            }
            let expr: String = src[start..i].iter().collect();
            push_value(&mut tokens, TokenType::Arith, expr);
            pos = i + 2;
            continue;
        }

        if c == '$' && next == Some('(') {
            let mut i = pos + 2;
            let start = i;
            let mut paren_depth = 1usize;
            let mut col = pos + 2;
            while i < len {
                match src[i] {
                    '(' => paren_depth += 1,
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
                col += 1;
            }
            if paren_depth != 0 {
                return Err(ShellError::lex("unfinished $(", col));
            }
            if word_continues_at(&src, i + 1) {
                let (word, quoted, after) = read_word(&src, pos)?;
                push_value(
                    &mut tokens,
                    if is_assignment_word(&word) {
                        TokenType::Assign
                    } else {
                        TokenType::Word
                    },
                    word,
                );
                tokens.last_mut().unwrap().quoted = quoted;
                pos = after;
                continue;
            }
            let inner: String = src[start..i].iter().collect();
            let full = format!("$({})", inner);
            tokens.push(Token {
                type_: TokenType::Word,
                value: Some(full),
                quoted: false,
            });
            pos = i + 1;
            continue;
        }

        if c == '$' {
            if next == Some('{') {
                let (var, after) = read_braced_variable(&src, pos + 2)?;
                push_value(&mut tokens, TokenType::VarBrace, var);
                pos = after;
                continue;
            }
            if let Some((name, after)) = read_variable_name(&src, pos + 1) {
                push_value(&mut tokens, TokenType::Var, name);
                pos = after;
                continue;
            }
        }

        if c == '(' {
            push_op(&mut tokens, TokenType::GroupOpen);
            pos += 1;
            continue;
        }
        if c == ')' {
            push_op(&mut tokens, TokenType::GroupClose);
            pos += 1;
            continue;
        }

        let (word, quoted, after) = read_word(&src, pos)?;
        let type_ = if is_assignment_word(&word) {
            TokenType::Assign
        } else {
            TokenType::Word
        };
        tokens.push(Token {
            type_,
            value: Some(word),
            quoted,
        });
        pos = after;
    }

    Ok(tokens)
}

fn is_assignment_word(word: &str) -> bool {
    let name_end = match word.find('=') {
        Some(i) => i,
        None => return false,
    };
    let name = &word[..name_end];
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types(line: &str) -> Vec<TokenType> {
        tokenize(line)
            .unwrap()
            .into_iter()
            .map(|t| t.type_)
            .collect()
    }

    fn values(line: &str) -> Vec<String> {
        tokenize(line)
            .unwrap()
            .into_iter()
            .map(|t| t.value.unwrap_or_default())
            .collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(tokenize("").unwrap().is_empty());
        assert!(tokenize("   \t  ").unwrap().is_empty());
    }

    #[test]
    fn simple_words() {
        assert_eq!(types("ls -la"), vec![TokenType::Word, TokenType::Word]);
        assert_eq!(values("ls -la"), vec!["ls", "-la"]);
    }

    #[test]
    fn single_quotes_group_without_expansion() {
        let toks = tokenize("'a b'").unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].type_, TokenType::Word);
        assert_eq!(toks[0].value.as_deref(), Some("'a b'"));
        assert!(toks[0].quoted);
    }

    #[test]
    fn unclosed_single_quote_is_error() {
        assert!(tokenize("'abc").is_err());
    }

    #[test]
    fn trailing_backslash_is_error() {
        assert!(tokenize("abc\\").is_err());
    }

    #[test]
    fn backslash_escape_makes_next_char_literal() {
        let toks = tokenize("a\\ b").unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].value.as_deref(), Some("a\\ b"));
        assert!(toks[0].quoted);
    }

    #[test]
    fn operators_are_detected() {
        assert_eq!(
            types("a | b"),
            vec![TokenType::Word, TokenType::Pipe, TokenType::Word]
        );
    }

    #[test]
    fn and_or_operators() {
        assert_eq!(
            types("a && b || c"),
            vec![
                TokenType::Word,
                TokenType::And,
                TokenType::Word,
                TokenType::Or,
                TokenType::Word
            ]
        );
    }

    #[test]
    fn redirections() {
        assert_eq!(
            types("cat < in > out"),
            vec![
                TokenType::Word,
                TokenType::RedirectIn,
                TokenType::Word,
                TokenType::RedirectOut,
                TokenType::Word
            ]
        );
    }

    #[test]
    fn append_and_heredoc() {
        assert_eq!(
            types("cmd >> f"),
            vec![TokenType::Word, TokenType::RedirectAppend, TokenType::Word]
        );
        assert_eq!(
            types("cmd << EOF"),
            vec![TokenType::Word, TokenType::RedirectHeredoc, TokenType::Word]
        );
        assert_eq!(
            types("cmd <<- EOF"),
            vec![
                TokenType::Word,
                TokenType::RedirectHeredocTab,
                TokenType::Word
            ]
        );
    }

    #[test]
    fn background_and_clobber() {
        assert_eq!(types("cmd &"), vec![TokenType::Word, TokenType::Background]);
        assert_eq!(
            types("cmd >| f"),
            vec![TokenType::Word, TokenType::RedirectClobber, TokenType::Word]
        );
        assert_eq!(
            types("cmd &> f"),
            vec![TokenType::Word, TokenType::RedirectBothOut, TokenType::Word]
        );
    }

    #[test]
    fn comments_start_at_word_boundary() {
        assert_eq!(types("ls # comment"), vec![TokenType::Word]);
        assert_eq!(types("# full comment"), vec![]);
    }

    #[test]
    fn hash_in_word_is_not_a_comment() {
        let toks = tokenize("a#b").unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].value.as_deref(), Some("a#b"));
    }

    #[test]
    fn variable_tokens() {
        assert_eq!(types("$HOME"), vec![TokenType::Var]);
        assert_eq!(values("$HOME"), vec!["HOME"]);
        assert_eq!(types("${PATH}"), vec![TokenType::VarBrace]);
        assert_eq!(values("${PATH}"), vec!["PATH"]);
    }

    #[test]
    fn special_variable_names() {
        assert_eq!(values("$?"), vec!["?"]);
        assert_eq!(values("$*"), vec!["*"]);
        assert_eq!(values("$@"), vec!["@"]);
        assert_eq!(values("$!"), vec!["!"]);
        assert_eq!(values("$1"), vec!["1"]);
    }

    #[test]
    fn unquoted_assignment_word_is_assign_token() {
        assert_eq!(types("x=5"), vec![TokenType::Assign]);
        assert_eq!(values("x=5"), vec!["x=5"]);
        assert_eq!(types("_name=value"), vec![TokenType::Assign]);
    }

    #[test]
    fn quoted_value_assignment_is_assign_token() {
        assert_eq!(types("x=\"a b\""), vec![TokenType::Assign]);
        assert_eq!(values("x=\"a b\""), vec!["x=\"a b\""]);
        assert_eq!(types("x='single'"), vec![TokenType::Assign]);
    }

    #[test]
    fn quoted_name_is_not_an_assignment() {
        assert_eq!(types("\"x\"=5"), vec![TokenType::Word]);
    }

    #[test]
    fn arithmetic_expansion() {
        assert_eq!(types("$(( 1 + 2 ))"), vec![TokenType::Arith]);
        assert_eq!(values("$(( 1 + 2 ))"), vec![" 1 + 2 "]);
    }

    #[test]
    fn unlimited_dollar_parens_are_errors() {
        assert!(tokenize("$(( 1 +").is_err());
    }

    #[test]
    fn command_substitution_is_a_word() {
        let toks = tokenize("$(echo hi)").unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].type_, TokenType::Word);
        assert_eq!(toks[0].value.as_deref(), Some("$(echo hi)"));
        assert!(!toks[0].quoted);
    }

    #[test]
    fn command_substitution_nested_is_captured_raw() {
        let line = "$(echo $(echo hi))";
        let toks = tokenize(line).unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].value.as_deref(), Some("$(echo $(echo hi))"));
    }

    #[test]
    fn unclosed_command_substitution_is_error() {
        assert!(tokenize("$(echo").is_err());
    }

    #[test]
    fn unclosed_brace_variable_is_error() {
        assert!(tokenize("${PATH").is_err());
    }

    #[test]
    fn single_char_dollar_is_word() {
        let toks = tokenize("$ x").unwrap();
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0].value.as_deref(), Some("$"));
    }
}
