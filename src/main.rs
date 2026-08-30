mod ast;
mod builtins;
mod error;
mod executor;
mod expansion;
mod lexer;
mod parser;
mod shell;
mod token;

use std::io::{self, BufRead, Write};

use ast::AstNode;
use error::ShellError;
use lexer::tokenize;
use shell::Shell;

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();

    let mut shell = Shell::new();

    loop {
        shell.reap_jobs();
        eprint!("eazyshell> ");
        if io::stderr().flush().is_err() {
            break;
        }

        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        match run_line(&line, &mut input, &mut shell) {
            Ok(status) => shell.last_status = status,
            Err(e) => {
                eprintln!("{}", e);
                shell.last_status = 1;
            }
        }

        if !shell.running {
            break;
        }
    }

    std::process::exit(shell.last_status);
}

fn collect_heredocs(
    node: &AstNode,
    input: &mut dyn BufRead,
    shell: &mut Shell,
) -> Result<(), ShellError> {
    match node {
        AstNode::Command { command } => {
            if let Some(delim) = &command.heredoc_delim {
                shell
                    .pending_heredocs
                    .push_back(read_heredoc_body(delim, input)?);
            }
            Ok(())
        }
        AstNode::Pipeline { pipeline } => {
            for cmd in &pipeline.commands {
                if let Some(delim) = &cmd.heredoc_delim {
                    shell
                        .pending_heredocs
                        .push_back(read_heredoc_body(delim, input)?);
                }
            }
            Ok(())
        }
        AstNode::And { left, right } => {
            collect_heredocs(left, input, shell)?;
            collect_heredocs(right, input, shell)
        }
        AstNode::Or { left, right } => {
            collect_heredocs(left, input, shell)?;
            collect_heredocs(right, input, shell)
        }
        AstNode::Semicolon { left, right } => {
            collect_heredocs(left, input, shell)?;
            collect_heredocs(right, input, shell)
        }
        AstNode::Group { body } => collect_heredocs(body, input, shell),
        AstNode::Background { body } => collect_heredocs(body, input, shell),
        AstNode::If(s) => {
            collect_heredocs(&s.condition, input, shell)?;
            collect_heredocs(&s.then_body, input, shell)?;
            if let Some(else_body) = &s.else_body {
                collect_heredocs(else_body, input, shell)?;
            }
            Ok(())
        }
        AstNode::While(s) => {
            collect_heredocs(&s.condition, input, shell)?;
            collect_heredocs(&s.body, input, shell)
        }
        AstNode::For(s) => collect_heredocs(&s.body, input, shell),
    }
}

fn read_heredoc_body(delim: &str, input: &mut dyn BufRead) -> Result<String, ShellError> {
    let mut body = String::new();
    loop {
        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) => return Err(ShellError::eval("heredoc: unexpected end of input")),
            Ok(_) => {}
            Err(e) => return Err(ShellError::eval(format!("heredoc: {}", e))),
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        if line == delim {
            break;
        }
        body.push_str(&line);
        body.push('\n');
    }
    Ok(body)
}

fn run_line(line: &str, input: &mut dyn BufRead, shell: &mut Shell) -> Result<i32, ShellError> {
    let tokens = tokenize(line)?;
    let ast = parser::parse(&tokens)?;
    collect_heredocs(&ast, input, shell)?;
    executor::eval(&ast, shell)
}
