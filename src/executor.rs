use std::fs::File;
use std::io::Write;
use std::process::{Command as ProcessCommand, Stdio};

use crate::ast::{AstNode, Command, Pipeline};
use crate::builtins::{is_builtin_name, run_builtin};
use crate::error::ShellError;
use crate::expansion::{expand_cmd_word, expand_word};
use crate::shell::{Shell, status_code};

pub fn eval(node: &AstNode, shell: &mut Shell) -> Result<i32, ShellError> {
    shell.reap_jobs();
    let status = match node {
        AstNode::Command { command } => eval_command(command, shell)?,
        AstNode::Pipeline { pipeline } => eval_pipeline(pipeline, shell)?,
        AstNode::And { left, right } => {
            let left_status = eval(left, shell)?;
            if left_status == 0 {
                eval(right, shell)?
            } else {
                left_status
            }
        }
        AstNode::Or { left, right } => {
            let left_status = eval(left, shell)?;
            if left_status != 0 {
                eval(right, shell)?
            } else {
                left_status
            }
        }
        AstNode::Semicolon { left, right } => {
            eval(left, shell)?;
            eval(right, shell)?
        }
        AstNode::Background { body } => eval_background(body, shell)?,
        AstNode::Group { body } => eval(body, shell)?,
        AstNode::If(s) => {
            let cond_status = eval(&s.condition, shell)?;
            if cond_status == 0 {
                eval(&s.then_body, shell)?
            } else if let Some(else_body) = &s.else_body {
                eval(else_body, shell)?
            } else {
                cond_status
            }
        }
        AstNode::While(s) => {
            let mut last = 0;
            while eval(&s.condition, shell)? == 0 {
                last = eval(&s.body, shell)?;
            }
            last
        }
        AstNode::For(s) => {
            let vars = expand_wordlist(&s.wordlist, shell)?;
            let mut last = 0;
            for v in vars {
                shell.set_var(&s.var, &v);
                last = eval(&s.body, shell)?;
                if !shell.running {
                    break;
                }
            }
            last
        }
    };
    shell.reap_jobs();
    shell.last_status = status;
    Ok(status)
}

fn expand_wordlist(raw: &[String], shell: &Shell) -> Result<Vec<String>, ShellError> {
    let mut out = Vec::new();
    for (i, word) in raw.iter().enumerate() {
        let fields = if i == 0 {
            expand_cmd_word(word, shell)?
        } else {
            expand_word(word, shell)?
        };
        for field in fields {
            out.push(field);
        }
    }
    Ok(out)
}

fn expand_argv(cmd: &Command, shell: &Shell) -> Result<Vec<String>, ShellError> {
    expand_wordlist(&cmd.argv, shell)
}

fn expand_target(raw: &str, shell: &Shell) -> Result<String, ShellError> {
    let fields = expand_word(raw, shell)?;
    match fields.first() {
        Some(f) => Ok(f.clone()),
        None => Ok(String::new()),
    }
}

fn open_input(raw: &str, shell: &Shell) -> Result<File, ShellError> {
    let path = expand_target(raw, shell)?;
    File::open(&path).map_err(|e| ShellError::eval(format!("{}: {}", path, e)))
}

fn open_output(raw: &str, append: bool, shell: &Shell) -> Result<File, ShellError> {
    let path = expand_target(raw, shell)?;
    let mut opts = File::options();
    opts.write(true);
    if append {
        opts.append(true);
    } else {
        opts.truncate(true);
    }
    opts.create(true);
    opts.open(&path)
        .map_err(|e| ShellError::eval(format!("{}: {}", path, e)))
}

fn configure_redirections(
    process: &mut ProcessCommand,
    cmd: &Command,
    shell: &mut Shell,
) -> Result<Option<String>, ShellError> {
    if let Some(src) = &cmd.infile {
        let file = open_input(src, shell)?;
        process.stdin(Stdio::from(file));
    }

    if let Some(dst) = &cmd.outfile {
        let file = open_output(dst, cmd.append_out, shell)?;
        process.stdout(Stdio::from(file));
    }

    let mut heredoc_body = None;
    if cmd.heredoc_delim.is_some() {
        heredoc_body = shell.pending_heredocs.pop_front();
        process.stdin(Stdio::piped());
    }

    Ok(heredoc_body)
}

fn spawn_child(
    process: &mut ProcessCommand,
    heredoc: Option<String>,
) -> Result<std::process::Child, ShellError> {
    let mut child = process
        .spawn()
        .map_err(|e| ShellError::eval(format!("failed to launch: {}", e)))?;
    if let Some(body) = heredoc
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(body.as_bytes());
        let _ = stdin.flush();
    }
    Ok(child)
}

fn build_process(name: &str, argv: &[String], shell: &Shell) -> ProcessCommand {
    let mut process = ProcessCommand::new(name);
    process.args(&argv[1..]);
    process.env_clear();
    for (k, v) in &shell.variables {
        process.env(k, v);
    }
    process
}

fn eval_background(body: &AstNode, shell: &mut Shell) -> Result<i32, ShellError> {
    match body {
        AstNode::Command { command } => {
            if !cmd_has_redirection(command) {
                apply_assignments(&command.env_assigns, shell)?;
                let argv = expand_argv(command, shell)?;
                let name = match argv.first() {
                    Some(n) => n.clone(),
                    None => {
                        shell.last_status = 0;
                        return Ok(0);
                    }
                };
                if is_builtin_name(&name) {
                    let status = run_builtin(&name, &argv, shell);
                    shell.last_status = status;
                    return Ok(status);
                }
                return launch_background(&name, &argv, command, shell);
            }
            Err(ShellError::eval(
                "redirection on builtins not yet implemented",
            ))
        }
        AstNode::Pipeline { pipeline } => {
            if pipeline.commands.len() == 1
                && !cmd_has_redirection(&pipeline.commands[0])
                && !is_builtin_name(
                    pipeline.commands[0]
                        .argv
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or(""),
                )
            {
                let cmd = &pipeline.commands[0];
                apply_assignments(&cmd.env_assigns, shell)?;
                let argv = expand_argv(cmd, shell)?;
                let name = match argv.first() {
                    Some(n) => n.clone(),
                    None => {
                        shell.last_status = 0;
                        return Ok(0);
                    }
                };
                return launch_background(&name, &argv, cmd, shell);
            }
            Err(ShellError::eval("background of pipeline not yet supported"))
        }
        _ => Err(ShellError::eval(
            "background of compound command not supported",
        )),
    }
}

fn launch_background(
    name: &str,
    argv: &[String],
    cmd: &Command,
    shell: &mut Shell,
) -> Result<i32, ShellError> {
    let mut process = build_process(name, argv, shell);
    let heredoc = configure_redirections(&mut process, cmd, shell)?;
    let child = spawn_child(&mut process, heredoc)?;
    let pid = child.id();
    let cmdline = argv.join(" ");
    shell.add_job(child, cmdline);
    shell.last_bg_pid = Some(pid as i32);
    shell.last_status = 0;
    Ok(0)
}

fn cmd_has_redirection(cmd: &Command) -> bool {
    cmd.infile.is_some() || cmd.outfile.is_some() || cmd.heredoc_delim.is_some()
}

fn run_builtin_with_redirection(
    name: &str,
    argv: &[String],
    cmd: &Command,
    shell: &mut Shell,
) -> Result<i32, ShellError> {
    if !cmd_has_redirection(cmd) {
        let status = run_builtin(name, argv, shell);
        shell.last_status = status;
        return Ok(status);
    }
    Err(ShellError::eval(
        "redirection on builtins not yet implemented",
    ))
}

fn apply_assignments(env_assigns: &[String], shell: &mut Shell) -> Result<(), ShellError> {
    for raw in env_assigns {
        let eq = match raw.find('=') {
            Some(i) => i,
            None => continue,
        };
        let name = &raw[..eq];
        let value_text = &raw[eq + 1..];
        let fields = expand_word(value_text, shell)?;
        let value = fields.join(" ");
        shell.set_var(name, &value);
    }
    Ok(())
}

fn eval_command(cmd: &Command, shell: &mut Shell) -> Result<i32, ShellError> {
    if cmd.is_empty() {
        shell.last_status = 0;
        return Ok(0);
    }

    apply_assignments(&cmd.env_assigns, shell)?;

    let argv = expand_argv(cmd, shell)?;
    let name = match argv.first() {
        Some(n) => n.clone(),
        None => {
            shell.last_status = 0;
            return Ok(0);
        }
    };

    if is_builtin_name(&name) {
        return run_builtin_with_redirection(&name, &argv, cmd, shell);
    }

    let mut process = build_process(&name, &argv, shell);
    let heredoc = configure_redirections(&mut process, cmd, shell)?;
    let mut child = spawn_child(&mut process, heredoc)?;
    let status = child
        .wait()
        .map_err(|e| ShellError::eval(format!("{}", e)))?;
    let code = status_code(status);
    shell.last_status = code;
    Ok(code)
}

fn eval_pipeline(pipeline: &Pipeline, shell: &mut Shell) -> Result<i32, ShellError> {
    let mut argvs: Vec<Vec<String>> = Vec::new();
    let mut cmds: Vec<&crate::ast::Command> = Vec::new();
    for (pos, cmd) in pipeline.commands.iter().enumerate() {
        if cmd.heredoc_delim.is_some() && pos > 0 {
            return Err(ShellError::eval(
                "heredoc on non-first pipeline command is not supported",
            ));
        }
        let argv = expand_argv(cmd, shell)?;
        if let Some(name) = argv.first() {
            if is_builtin_name(name) {
                return Err(ShellError::eval("builtins in pipelines not yet supported"));
            }
            cmds.push(cmd);
            argvs.push(argv);
        }
    }

    let n = argvs.len();
    if n == 0 {
        return Ok(0);
    }
    if n == 1 {
        let mut process = build_process(&argvs[0][0], &argvs[0], shell);
        let heredoc = configure_redirections(&mut process, cmds[0], shell)?;
        let mut child = spawn_child(&mut process, heredoc)?;
        let status = child
            .wait()
            .map_err(|e| ShellError::eval(format!("{}", e)))?;
        let code = status_code(status);
        shell.last_status = code;
        return Ok(code);
    }

    let mut processes: Vec<(ProcessCommand, Option<String>)> = Vec::with_capacity(n);
    for idx in 0..n {
        let argv = &argvs[idx];
        let mut process = build_process(&argv[0], argv, shell);
        let heredoc = configure_redirections(&mut process, cmds[idx], shell)?;
        processes.push((process, heredoc));
    }

    let mut children: Vec<std::process::Child> = Vec::with_capacity(n);

    let mut spawn_error: Option<ShellError> = None;
    for idx in 0..n {
        if spawn_error.is_some() {
            break;
        }
        let (mut process, heredoc) =
            std::mem::replace(&mut processes[idx], (ProcessCommand::new(""), None));
        if idx > 0 {
            process.stdin(
                children[idx - 1]
                    .stdout
                    .take()
                    .map(Stdio::from)
                    .unwrap_or(Stdio::null()),
            );
        }
        if idx + 1 < n {
            process.stdout(Stdio::piped());
        }
        let mut child = match process.spawn() {
            Ok(c) => c,
            Err(e) => {
                spawn_error = Some(ShellError::eval(format!("failed to launch: {}", e)));
                break;
            }
        };
        if let Some(body) = heredoc
            && let Some(mut stdin) = child.stdin.take()
        {
            let _ = stdin.write_all(body.as_bytes());
            let _ = stdin.flush();
        }
        children.push(child);
    }

    let mut last_code = 0;
    for mut child in children {
        let status = child.wait();
        match status {
            Ok(st) => last_code = status_code(st),
            Err(e) => return Err(ShellError::eval(format!("{}", e))),
        }
    }

    if let Some(err) = spawn_error {
        return Err(err);
    }

    Ok(last_code)
}
