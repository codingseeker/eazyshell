use crate::ast::{AstNode, Command, Pipeline};
use crate::error::ShellError;
use crate::token::{Token, TokenType};

const MAX_PIPE_CMDS: usize = 1024;
const MAX_PARSE_DEPTH: usize = 256;

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Parser {
            tokens,
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_type(&self) -> Option<TokenType> {
        self.peek().map(|t| t.type_)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn near(&self) -> String {
        self.peek()
            .map(|t| t.text().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "end of input".to_string())
    }
}

fn is_command_terminator(t: TokenType) -> bool {
    matches!(
        t,
        TokenType::Pipe
            | TokenType::And
            | TokenType::Or
            | TokenType::Semicolon
            | TokenType::Background
            | TokenType::GroupClose
    )
}

const RESERVED: &[&str] = &[
    "if", "then", "else", "elif", "fi", "while", "do", "done", "for", "in",
];

fn peek_word(p: &Parser) -> Option<String> {
    match p.peek() {
        Some(Token {
            type_: TokenType::Word,
            value: Some(v),
            ..
        }) => Some(v.clone()),
        _ => None,
    }
}

fn is_reserved(p: &Parser) -> bool {
    match peek_word(p) {
        Some(w) => RESERVED.contains(&w.as_str()),
        None => false,
    }
}

fn is_stop(p: &Parser, stops: &[&str]) -> bool {
    match peek_word(p) {
        Some(w) => stops.contains(&w.as_str()),
        None => false,
    }
}

fn parse_command(p: &mut Parser) -> Result<Command, ShellError> {
    let mut cmd = Command::new();

    while let Some(tok) = p.peek() {
        if is_command_terminator(tok.type_) {
            break;
        }
        let t = tok.clone();
        match t.type_ {
            TokenType::Word => {
                cmd.argv.push(t.text().to_string());
                p.advance();
            }
            TokenType::Assign => {
                if cmd.argv.is_empty() {
                    cmd.env_assigns.push(t.text().to_string());
                    p.advance();
                } else {
                    cmd.argv.push(t.text().to_string());
                    p.advance();
                }
            }
            TokenType::Arith => {
                cmd.argv.push(format!("$(({}))", t.text()));
                p.advance();
            }
            TokenType::Var => {
                cmd.argv.push(format!("${}", t.text()));
                p.advance();
            }
            TokenType::VarBrace => {
                cmd.argv.push(format!("${{{}}}", t.text()));
                p.advance();
            }
            TokenType::RedirectIn => {
                p.advance();
                cmd.infile = Some(expect_word(p)?);
            }
            TokenType::RedirectOut => {
                p.advance();
                cmd.outfile = Some(expect_word(p)?);
                cmd.append_out = false;
            }
            TokenType::RedirectAppend => {
                p.advance();
                cmd.outfile = Some(expect_word(p)?);
                cmd.append_out = true;
            }
            TokenType::RedirectHeredoc => {
                p.advance();
                cmd.heredoc_delim = Some(expect_word(p)?);
            }
            _ => break,
        }
    }

    Ok(cmd)
}

fn expect_word(p: &mut Parser) -> Result<String, ShellError> {
    match p.peek() {
        Some(Token {
            type_: TokenType::Word,
            value: Some(v),
            ..
        }) => {
            let v = v.clone();
            p.advance();
            Ok(v)
        }
        _ => Err(ShellError::parse("expected a word", Some(p.near()))),
    }
}

fn parse_component(p: &mut Parser) -> Result<Option<AstNode>, ShellError> {
    if p.depth > MAX_PARSE_DEPTH {
        return Err(ShellError::parse("input nested too deeply", Some(p.near())));
    }
    match p.peek_type() {
        Some(TokenType::GroupOpen) => Ok(Some(parse_group(p)?)),
        _ => {
            if let Some(w) = peek_word(p)
                && RESERVED.contains(&w.as_str())
            {
                match w.as_str() {
                    "if" => return Ok(Some(parse_if(p)?)),
                    "while" => return Ok(Some(parse_while(p)?)),
                    "for" => return Ok(Some(parse_for(p)?)),
                    _ => return Ok(None),
                }
            }
            parse_simple(p)
        }
    }
}

fn parse_simple(p: &mut Parser) -> Result<Option<AstNode>, ShellError> {
    let first = parse_command(p)?;
    if first.is_empty() {
        return Err(ShellError::parse("empty command", Some(p.near())));
    }
    if p.peek_type() != Some(TokenType::Pipe) {
        return Ok(Some(AstNode::command(first)));
    }
    let mut pipeline = Pipeline::new();
    pipeline.commands.push(first);
    while p.peek_type() == Some(TokenType::Pipe) {
        if pipeline.commands.len() >= MAX_PIPE_CMDS {
            return Err(ShellError::parse("pipeline too long", Some(p.near())));
        }
        p.advance();
        let next = parse_command(p)?;
        if next.is_empty() {
            return Err(ShellError::parse(
                "expected a command in pipeline",
                Some(p.near()),
            ));
        }
        pipeline.commands.push(next);
    }
    Ok(Some(AstNode::pipeline(pipeline)))
}

fn background_last(node: AstNode) -> AstNode {
    match node {
        AstNode::Semicolon { left, right } => {
            AstNode::binary_semicolon(*left, background_last(*right))
        }
        other => AstNode::background(other),
    }
}

fn parse_list_until(p: &mut Parser, stops: &[&str]) -> Result<AstNode, ShellError> {
    let first = match parse_component(p)? {
        Some(node) => node,
        None => return Ok(AstNode::command(Command::new())),
    };
    let mut left = first;

    loop {
        if is_stop(p, stops) {
            break;
        }
        match p.peek_type() {
            Some(TokenType::Semicolon) => {
                p.advance();
                if is_stop(p, stops) {
                    break;
                }
                let right = match parse_component(p)? {
                    Some(n) => n,
                    None => break,
                };
                left = AstNode::binary_semicolon(left, right);
            }
            Some(TokenType::And) => {
                p.advance();
                if is_stop(p, stops) {
                    break;
                }
                let right = match parse_component(p)? {
                    Some(n) => n,
                    None => break,
                };
                left = AstNode::binary_and(left, right);
            }
            Some(TokenType::Or) => {
                p.advance();
                if is_stop(p, stops) {
                    break;
                }
                let right = match parse_component(p)? {
                    Some(n) => n,
                    None => break,
                };
                left = AstNode::binary_or(left, right);
            }
            Some(TokenType::Background) => {
                p.advance();
                left = background_last(left);
                if is_stop(p, stops) || p.peek().is_none() {
                    break;
                }
                let right = match parse_component(p)? {
                    Some(n) => n,
                    None => break,
                };
                left = AstNode::binary_semicolon(left, right);
            }
            _ => break,
        }
    }

    Ok(left)
}

fn parse_if(p: &mut Parser) -> Result<AstNode, ShellError> {
    p.advance();
    let condition = parse_list_until(p, &["then"])?;
    if !peek_word(p)
        .as_deref()
        .map(|w| w == "then")
        .unwrap_or(false)
    {
        return Err(ShellError::parse("expected 'then'", Some(p.near())));
    }
    p.advance();

    let then_body = parse_list_until(p, &["else", "elif", "fi"])?;

    let else_body = parse_if_tail(p)?;

    if !peek_word(p).as_deref().map(|w| w == "fi").unwrap_or(false) {
        return Err(ShellError::parse("expected 'fi'", Some(p.near())));
    }
    p.advance();

    Ok(AstNode::If(crate::ast::IfStmt {
        condition: Box::new(condition),
        then_body: Box::new(then_body),
        else_body: else_body.map(Box::new),
    }))
}

fn parse_if_tail(p: &mut Parser) -> Result<Option<AstNode>, ShellError> {
    match peek_word(p).as_deref() {
        Some("elif") => {
            p.advance();
            let condition = parse_list_until(p, &["then"])?;
            if !peek_word(p)
                .as_deref()
                .map(|w| w == "then")
                .unwrap_or(false)
            {
                return Err(ShellError::parse("expected 'then'", Some(p.near())));
            }
            p.advance();
            let then_body = parse_list_until(p, &["else", "elif", "fi"])?;
            let nested_else = parse_if_tail(p)?;
            Ok(Some(AstNode::If(crate::ast::IfStmt {
                condition: Box::new(condition),
                then_body: Box::new(then_body),
                else_body: nested_else.map(Box::new),
            })))
        }
        Some("else") => {
            p.advance();
            Ok(Some(parse_list_until(p, &["fi"])?))
        }
        _ => Ok(None),
    }
}

fn parse_while(p: &mut Parser) -> Result<AstNode, ShellError> {
    p.advance();
    let condition = parse_list_until(p, &["do"])?;
    if !peek_word(p).as_deref().map(|w| w == "do").unwrap_or(false) {
        return Err(ShellError::parse("expected 'do'", Some(p.near())));
    }
    p.advance();
    let body = parse_list_until(p, &["done"])?;
    if !peek_word(p)
        .as_deref()
        .map(|w| w == "done")
        .unwrap_or(false)
    {
        return Err(ShellError::parse("expected 'done'", Some(p.near())));
    }
    p.advance();
    Ok(AstNode::While(crate::ast::WhileStmt {
        condition: Box::new(condition),
        body: Box::new(body),
    }))
}

fn parse_for(p: &mut Parser) -> Result<AstNode, ShellError> {
    p.advance();
    let var = match peek_word(p) {
        Some(v) => {
            let v = v.clone();
            p.advance();
            v
        }
        None => return Err(ShellError::parse("expected loop variable", Some(p.near()))),
    };

    let mut wordlist = Vec::new();
    if peek_word(p).as_deref() == Some("in") {
        p.advance();
        while let Some(w) = peek_word(p) {
            if RESERVED.contains(&w.as_str()) {
                break;
            }
            wordlist.push(w);
            p.advance();
        }
    }

    if p.peek_type() == Some(TokenType::Semicolon) {
        p.advance();
    }

    if !peek_word(p).as_deref().map(|w| w == "do").unwrap_or(false) {
        return Err(ShellError::parse("expected 'do'", Some(p.near())));
    }
    p.advance();
    let body = parse_list_until(p, &["done"])?;
    if !peek_word(p)
        .as_deref()
        .map(|w| w == "done")
        .unwrap_or(false)
    {
        return Err(ShellError::parse("expected 'done'", Some(p.near())));
    }
    p.advance();

    Ok(AstNode::For(crate::ast::ForStmt {
        var,
        wordlist,
        body: Box::new(body),
    }))
}

fn parse_group(p: &mut Parser) -> Result<AstNode, ShellError> {
    if p.depth > MAX_PARSE_DEPTH {
        return Err(ShellError::parse("input nested too deeply", Some(p.near())));
    }
    p.advance();

    let body = parse_list_until(p, &[])?;

    if p.peek_type() != Some(TokenType::GroupClose) {
        return Err(ShellError::parse("expected ')'", Some(p.near())));
    }
    p.advance();

    Ok(AstNode::group(body))
}

pub fn parse(tokens: &[Token]) -> Result<AstNode, ShellError> {
    let mut p = Parser::new(tokens);
    parse_toplevel(&mut p)
}

fn parse_toplevel(p: &mut Parser) -> Result<AstNode, ShellError> {
    if p.depth > MAX_PARSE_DEPTH {
        return Err(ShellError::parse("input nested too deeply", Some(p.near())));
    }
    let node = parse_list_until(p, &[])?;
    if p.peek().is_some() && !is_reserved(p) {
        return Err(ShellError::parse("unexpected token", Some(p.near())));
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_ok(line: &str) -> AstNode {
        let tokens = tokenize(line).expect("lexer error");
        parse(&tokens).expect("parse error")
    }

    #[test]
    fn parses_simple_command() {
        let node = parse_ok("ls -la");
        match node {
            AstNode::Command { command } => {
                assert_eq!(command.argv, vec!["ls", "-la"]);
                assert!(command.infile.is_none());
                assert!(command.outfile.is_none());
            }
            other => panic!("expected command, got {:?}", other),
        }
    }

    #[test]
    fn arith_token_becomes_command_arg() {
        let node = parse_ok("echo $(( 1 + 2 ))");
        match node {
            AstNode::Command { command } => {
                assert_eq!(command.argv, vec!["echo", "$(( 1 + 2 ))"]);
            }
            other => panic!("expected command, got {:?}", other),
        }
    }

    #[test]
    fn parses_redirections() {
        let node = parse_ok("cat < in > out");
        match node {
            AstNode::Command { command } => {
                assert_eq!(command.argv, vec!["cat"]);
                assert_eq!(command.infile.as_deref(), Some("in"));
                assert_eq!(command.outfile.as_deref(), Some("out"));
                assert!(!command.append_out);
            }
            other => panic!("expected command, got {:?}", other),
        }
    }

    #[test]
    fn parses_append_redirection() {
        let node = parse_ok("cmd >> log");
        match node {
            AstNode::Command { command } => {
                assert_eq!(command.outfile.as_deref(), Some("log"));
                assert!(command.append_out);
            }
            other => panic!("expected command, got {:?}", other),
        }
    }

    #[test]
    fn heredoc_sets_delim() {
        let node = parse_ok("cmd << EOF");
        match node {
            AstNode::Command { command } => {
                assert_eq!(command.heredoc_delim.as_deref(), Some("EOF"));
            }
            other => panic!("expected command, got {:?}", other),
        }
    }

    #[test]
    fn parses_pipeline() {
        let node = parse_ok("a | b | c");
        match node {
            AstNode::Pipeline { pipeline } => {
                assert_eq!(pipeline.commands.len(), 3);
                assert_eq!(pipeline.commands[0].argv, vec!["a"]);
                assert_eq!(pipeline.commands[1].argv, vec!["b"]);
                assert_eq!(pipeline.commands[2].argv, vec!["c"]);
            }
            other => panic!("expected pipeline, got {:?}", other),
        }
    }

    #[test]
    fn empty_command_is_error() {
        let toks = tokenize("|").unwrap();
        assert!(parse(&toks).is_err());
    }

    #[test]
    fn parses_and_or_semicolon() {
        let node = parse_ok("a && b || c ; d");
        match node {
            AstNode::Semicolon { left, right } => {
                assert_eq!(
                    *left,
                    AstNode::binary_or(
                        AstNode::binary_and(
                            AstNode::command(Command {
                                argv: vec!["a".into()],
                                ..Default::default()
                            }),
                            AstNode::command(Command {
                                argv: vec!["b".into()],
                                ..Default::default()
                            }),
                        ),
                        AstNode::command(Command {
                            argv: vec!["c".into()],
                            ..Default::default()
                        }),
                    )
                );
                assert_eq!(
                    *right,
                    AstNode::command(Command {
                        argv: vec!["d".into()],
                        ..Default::default()
                    })
                );
            }
            other => panic!("expected semicolon, got {:?}", other),
        }
    }

    #[test]
    fn parses_group() {
        let node = parse_ok("cmd && ( a | b )");
        match node {
            AstNode::And { left, right } => {
                assert_eq!(
                    *left,
                    AstNode::command(Command {
                        argv: vec!["cmd".into()],
                        ..Default::default()
                    })
                );
                match *right {
                    AstNode::Group { body } => match *body {
                        AstNode::Pipeline { pipeline } => {
                            assert_eq!(pipeline.commands.len(), 2);
                        }
                        other => panic!("expected pipeline in group, got {:?}", other),
                    },
                    other => panic!("expected group, got {:?}", other),
                }
            }
            other => panic!("expected and, got {:?}", other),
        }
    }

    #[test]
    fn trailing_operator_produces_error() {
        let toks = tokenize("a &&").unwrap();
        assert!(parse(&toks).is_err());
    }

    #[test]
    fn pipe_missing_rhs_is_error() {
        let toks = tokenize("a |").unwrap();
        assert!(parse(&toks).is_err());
    }

    #[test]
    fn background_separates_commands() {
        let node = parse_ok("a & b");
        match node {
            AstNode::Semicolon { left, right } => {
                assert_eq!(
                    *left,
                    AstNode::background(AstNode::command(Command {
                        argv: vec!["a".into()],
                        ..Default::default()
                    }))
                );
                assert_eq!(
                    *right,
                    AstNode::command(Command {
                        argv: vec!["b".into()],
                        ..Default::default()
                    })
                );
            }
            other => panic!("expected semicolon, got {:?}", other),
        }
    }
}
