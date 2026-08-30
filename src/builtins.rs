use std::path::Path;

use crate::shell::Shell;

pub fn is_builtin_name(name: &str) -> bool {
    matches!(
        name,
        "cd" | "exit"
            | "pwd"
            | "export"
            | "unset"
            | "echo"
            | "alias"
            | "type"
            | "jobs"
            | "fg"
            | "bg"
    )
}

pub fn run_builtin(name: &str, args: &[String], shell: &mut Shell) -> i32 {
    match name {
        "cd" => do_cd(args, shell),
        "exit" => do_exit(args, shell),
        "pwd" => do_pwd(),
        "export" => do_export(args, shell),
        "unset" => do_unset(args, shell),
        "echo" => do_echo(args),
        "alias" => do_alias(args, shell),
        "type" => do_type(args, shell),
        "jobs" => do_jobs(shell),
        "fg" => do_fg(args, shell),
        "bg" => do_bg(args, shell),
        _ => {
            eprintln!("eazyshell: {}: not a builtin", name);
            1
        }
    }
}

fn job_status_label(status: &crate::shell::JobStatus) -> String {
    match status {
        crate::shell::JobStatus::Running => "Running".to_string(),
        crate::shell::JobStatus::Done(code) => format!("Done({})", code),
    }
}

fn do_jobs(shell: &mut Shell) -> i32 {
    shell.reap_jobs();
    for job in &shell.jobs {
        println!(
            "[{}] {} {}",
            job.id,
            job_status_label(&job.status),
            job.cmdline
        );
    }
    0
}

fn select_job_index(shell: &Shell, spec: &str) -> Option<usize> {
    if spec.is_empty() {
        return if shell.jobs.is_empty() {
            None
        } else {
            Some(shell.jobs.len() - 1)
        };
    }
    let digits: String = spec
        .trim_start_matches('%')
        .trim_start_matches('+')
        .trim_start_matches('-')
        .to_string();
    match digits.parse::<usize>() {
        Ok(n) => {
            for (i, job) in shell.jobs.iter().enumerate() {
                if job.id == n || job.child.id() as usize == n {
                    return Some(i);
                }
            }
            if n == shell.jobs.len() {
                Some(shell.jobs.len() - 1)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn do_fg(args: &[String], shell: &mut Shell) -> i32 {
    shell.reap_jobs();
    let spec = args.get(1).map(|s| s.as_str()).unwrap_or("");
    let idx = match select_job_index(shell, spec) {
        Some(i) => i,
        None => {
            eprintln!("eazyshell: fg: no such job");
            return 1;
        }
    };
    let mut job = shell.jobs.remove(idx);
    let cmdline = job.cmdline.clone();
    match job.child.wait() {
        Ok(status) => {
            let code = crate::shell::status_code(status);
            eprintln!("{} done with status {}", cmdline, code);
            code
        }
        Err(e) => {
            eprintln!("eazyshell: fg: {}", e);
            1
        }
    }
}

fn do_bg(args: &[String], shell: &mut Shell) -> i32 {
    shell.reap_jobs();
    let spec = args.get(1).map(|s| s.as_str()).unwrap_or("");
    let idx = match select_job_index(shell, spec) {
        Some(i) => i,
        None => {
            eprintln!("eazyshell: bg: no such job");
            return 1;
        }
    };
    let job = &shell.jobs[idx];
    match job.status {
        crate::shell::JobStatus::Running => {
            eprintln!("[{}] {} already running", job.id, job.cmdline);
            0
        }
        crate::shell::JobStatus::Done(_) => {
            eprintln!("eazyshell: bg: resumed stopped jobs not supported in safe std");
            1
        }
    }
}

fn do_exit(args: &[String], shell: &mut Shell) -> i32 {
    let status = args
        .get(1)
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(shell.last_status);
    shell.running = false;
    status
}

fn do_cd(args: &[String], shell: &mut Shell) -> i32 {
    let target = match args.get(1) {
        Some(d) if d == "~" => shell.home(),
        Some(d) if d == "-" => shell.oldpwd.clone(),
        Some(d) => Some(d.clone()),
        None => shell.home(),
    };

    let dir = match target {
        Some(d) => d,
        None => {
            eprintln!("eazyshell: cd: no directory given");
            return 1;
        }
    };

    let current = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("eazyshell: cd: {}", e);
            return 1;
        }
    };

    match std::env::set_current_dir(&dir) {
        Ok(()) => {
            shell.oldpwd = Some(current.to_string_lossy().into_owned());
            0
        }
        Err(e) => {
            eprintln!("eazyshell: cd: {}: {}", dir, e);
            1
        }
    }
}

fn do_pwd() -> i32 {
    match std::env::current_dir() {
        Ok(cwd) => {
            println!("{}", cwd.display());
            0
        }
        Err(e) => {
            eprintln!("eazyshell: pwd: {}", e);
            1
        }
    }
}

fn do_export(args: &[String], shell: &mut Shell) -> i32 {
    if args.len() < 2 {
        for (name, value) in &shell.variables {
            println!("{}={}", name, value);
        }
        return 0;
    }
    for arg in &args[1..] {
        match arg.split_once('=') {
            Some((name, value)) => shell.set_var(name, value),
            None => shell.set_var(arg, ""),
        }
    }
    0
}

fn do_unset(args: &[String], shell: &mut Shell) -> i32 {
    for arg in &args[1..] {
        shell.unset_var(arg);
    }
    0
}

fn do_echo(args: &[String]) -> i32 {
    let mut start = 1usize;
    let mut newline = true;
    if args.get(1).map(|s| s.as_str()) == Some("-n") {
        newline = false;
        start = 2;
    }
    for (i, arg) in args[start..].iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", arg);
    }
    if newline {
        println!();
    } else {
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    0
}

fn do_alias(args: &[String], shell: &mut Shell) -> i32 {
    if args.len() < 2 {
        for (name, value) in &shell.aliases {
            println!("alias {}='{}'", name, value);
        }
        return 0;
    }
    for arg in &args[1..] {
        match arg.split_once('=') {
            Some((name, value)) => shell.set_alias(name, value),
            None => match shell.get_alias(arg) {
                Some(value) => println!("alias {}='{}'", arg, value),
                None => eprintln!("eazyshell: alias: {}: not found", arg),
            },
        }
    }
    0
}

fn do_type(args: &[String], shell: &mut Shell) -> i32 {
    let Some(arg) = args.get(1) else {
        return 0;
    };
    if is_builtin_name(arg) {
        println!("{} is a shell builtin", arg);
        return 0;
    }
    if let Some(value) = shell.get_alias(arg) {
        println!("{} is aliased to '{}'", arg, value);
        return 0;
    }
    if let Some(path) = find_in_path(arg, shell) {
        println!("{} is {}", arg, path);
        return 0;
    }
    println!("{} not found", arg);
    1
}

fn find_in_path(name: &str, shell: &Shell) -> Option<String> {
    let path = shell.get_var("PATH").map(|s| s.to_string());
    let candidates: Vec<&str> = match &path {
        Some(p) => p.split(':').collect(),
        None => vec!["/usr/local/bin", "/usr/bin", "/bin"],
    };
    for dir in candidates {
        if dir.is_empty() {
            continue;
        }
        let full = Path::new(dir).join(name);
        if let Ok(meta) = std::fs::metadata(&full) {
            use std::os::unix::fs::MetadataExt;
            if meta.is_file() {
                let mode = meta.mode();
                if mode & 0o111 != 0 {
                    return Some(full.to_string_lossy().into_owned());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_with(vars: &[(&str, &str)]) -> Shell {
        let mut s = Shell::new();
        s.variables.clear();
        for (k, v) in vars {
            s.variables.push((k.to_string(), v.to_string()));
        }
        s
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn recognizes_builtin_names() {
        assert!(is_builtin_name("cd"));
        assert!(is_builtin_name("exit"));
        assert!(is_builtin_name("echo"));
        assert!(is_builtin_name("type"));
        assert!(!is_builtin_name("ls"));
    }

    #[test]
    fn export_without_args_prints_all() {
        let mut s = shell_with(&[("A", "1"), ("B", "2")]);
        let rc = run_builtin("export", &["export".to_string()], &mut s);
        assert_eq!(rc, 0);
    }

    #[test]
    fn export_sets_variable() {
        let mut s = shell_with(&[]);
        let rc = run_builtin("export", &argv(&["export", "FOO=bar", "EMPTY"]), &mut s);
        assert_eq!(rc, 0);
        assert_eq!(s.get_var("FOO"), Some("bar"));
        assert_eq!(s.get_var("EMPTY"), Some(""));
    }

    #[test]
    fn unset_removes_variable() {
        let mut s = shell_with(&[("X", "1")]);
        let rc = run_builtin("unset", &argv(&["unset", "X"]), &mut s);
        assert_eq!(rc, 0);
        assert_eq!(s.get_var("X"), None);
    }

    #[test]
    fn echo_without_n_flag_adds_newline() {
        let mut s = shell_with(&[]);
        let rc = run_builtin("echo", &argv(&["echo", "hi"]), &mut s);
        assert_eq!(rc, 0);
    }

    #[test]
    fn alias_sets_and_lists() {
        let mut s = shell_with(&[]);
        assert_eq!(
            run_builtin("alias", &argv(&["alias", "ll=ls -l"]), &mut s),
            0
        );
        assert_eq!(s.get_alias("ll"), Some("ls -l"));
        assert_eq!(run_builtin("alias", &argv(&["alias", "ll"]), &mut s), 0);
    }

    #[test]
    fn type_reports_builtin() {
        let mut s = shell_with(&[]);
        assert_eq!(run_builtin("type", &argv(&["type", "echo"]), &mut s), 0);
    }

    #[test]
    fn type_not_found_returns_nonzero() {
        let mut s = shell_with(&[]);
        assert_ne!(
            run_builtin("type", &argv(&["type", "definitely_not_a_cmd"]), &mut s),
            0
        );
    }

    #[test]
    fn cd_without_home_is_error() {
        let mut s = shell_with(&[]);
        let rc = run_builtin("cd", &argv(&["cd"]), &mut s);
        assert_eq!(rc, 1);
    }
}
