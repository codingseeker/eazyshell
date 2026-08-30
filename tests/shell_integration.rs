use std::io::Write;
use std::process::{Command, Stdio};

fn run_shell(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eazyshell"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn eazyshell");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().expect("failed to wait on shell");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn shell_exit_code(input: &str) -> i32 {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eazyshell"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn eazyshell");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    child
        .wait()
        .expect("failed to wait on shell")
        .code()
        .unwrap_or(-1)
}

#[test]
fn reports_prompt_and_echo_output() {
    let out = run_shell("echo hello\nexit\n");
    assert!(out.contains("hello"), "output was: {:?}", out);
}

#[test]
fn echo_dash_n_suppresses_newline() {
    let out = run_shell("echo -n ab; echo cd\nexit\n");
    assert!(out.contains("abcd"), "output was: {:?}", out);
}

#[test]
fn external_commands_run() {
    let out = run_shell("/bin/echo external-ok\nexit\n");
    assert!(out.contains("external-ok"), "output was: {:?}", out);
}

#[test]
fn command_not_found_reports_error_but_keeps_going() {
    let out = run_shell("definitely_not_a_real_command_xyz\necho after\nexit\n");
    assert!(out.contains("after"), "output was: {:?}", out);
}

#[test]
fn lexer_error_keeps_shell_alive() {
    let out = run_shell("echo \"unclosed\necho after-error\nexit\n");
    assert!(out.contains("after-error"), "output was: {:?}", out);
}

#[test]
fn and_or_operator_short_circuits() {
    let out = run_shell(
        "/bin/true && echo yes\n/bin/false && echo nope\n/bin/false || echo fallback\nexit\n",
    );
    assert!(out.contains("yes"), "output was: {:?}", out);
    assert!(!out.contains("nope"), "output was: {:?}", out);
    assert!(out.contains("fallback"), "output was: {:?}", out);
}

#[test]
fn semicolon_runs_in_sequence() {
    let out = run_shell("echo one; echo two; echo three\nexit\n");
    assert!(out.contains("one"), "output was: {:?}", out);
    assert!(out.contains("two"), "output was: {:?}", out);
    assert!(out.contains("three"), "output was: {:?}", out);
}

#[test]
fn export_and_parameter_expansion() {
    let out = run_shell("export GREETING=hi\necho $GREETING world\nexit\n");
    assert!(out.contains("hi world"), "output was: {:?}", out);
}

#[test]
fn pipe_connects_commands() {
    let out = run_shell("/bin/echo piped | /bin/cat\nexit\n");
    assert!(out.contains("piped"), "output was: {:?}", out);
}

#[test]
fn multi_stage_pipeline() {
    let out = run_shell("/bin/printf 'b\\na\\nc\\n' | /bin/sort | /bin/cat\nexit\n");
    assert!(out.contains("a"), "output was: {:?}", out);
    assert!(out.contains("b"), "output was: {:?}", out);
    assert!(out.contains("c"), "output was: {:?}", out);
}

#[test]
fn exit_sets_last_status() {
    let out = run_shell("/bin/false\necho $?\nexit\n");
    assert!(out.contains("1"), "output was: {:?}", out);
}

#[test]
fn output_redirection_to_file() {
    std::fs::remove_file("/tmp/eazyshell_out_test.txt").ok();
    let script = "/bin/echo redirected > /tmp/eazyshell_out_test.txt\nexit\n";
    run_shell(script);
    let content = std::fs::read_to_string("/tmp/eazyshell_out_test.txt").unwrap();
    assert!(content.contains("redirected"), "file was: {:?}", content);
    std::fs::remove_file("/tmp/eazyshell_out_test.txt").ok();
}

#[test]
fn append_redirection() {
    std::fs::remove_file("/tmp/eazyshell_append_test.txt").ok();
    let script = "/bin/echo one >> /tmp/eazyshell_append_test.txt\n/bin/echo two >> /tmp/eazyshell_append_test.txt\nexit\n";
    run_shell(script);
    let content = std::fs::read_to_string("/tmp/eazyshell_append_test.txt").unwrap();
    assert!(content.contains("one"), "file was: {:?}", content);
    assert!(content.contains("two"), "file was: {:?}", content);
    std::fs::remove_file("/tmp/eazyshell_append_test.txt").ok();
}

#[test]
fn input_redirection_from_file() {
    std::fs::write("/tmp/eazyshell_in_test.txt", "from-file\n").unwrap();
    let out = run_shell("/bin/cat < /tmp/eazyshell_in_test.txt\nexit\n");
    assert!(out.contains("from-file"), "output was: {:?}", out);
    std::fs::remove_file("/tmp/eazyshell_in_test.txt").ok();
}

#[test]
fn heredoc_feeds_stdin() {
    let out = run_shell("/bin/cat << EOF\nline one\nline two\nEOF\nexit\n");
    assert!(out.contains("line one"), "output was: {:?}", out);
    assert!(out.contains("line two"), "output was: {:?}", out);
}

#[test]
fn heredoc_on_first_pipeline_command_feeds_stdin() {
    let out = run_shell("/bin/cat << EOF | /bin/tr a-z A-Z\nhello world\nEOF\nexit\n");
    assert!(out.contains("HELLO WORLD"), "output was: {:?}", out);
}

#[test]
fn heredoc_on_non_first_pipeline_command_is_rejected() {
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_eazyshell"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn eazyshell");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"/bin/echo hi | /bin/cat << EOF\nbody\nEOF\nexit\n")
        .unwrap();
    let output = child.wait_with_output().expect("failed to wait on shell");
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        err.contains("heredoc on non-first pipeline command is not supported"),
        "stderr was: {:?}",
        err
    );
}

#[test]
fn arithmetic_expansion() {
    let out = run_shell("echo $(( 6 * 7 ))\nexit\n");
    assert!(out.contains("42"), "output was: {:?}", out);
}

#[test]
fn brace_expansion() {
    let out = run_shell("echo file{A,B,C}.txt\nexit\n");
    assert!(out.contains("fileA.txt"), "output was: {:?}", out);
    assert!(out.contains("fileB.txt"), "output was: {:?}", out);
    assert!(out.contains("fileC.txt"), "output was: {:?}", out);
}

#[test]
fn word_splitting_on_unquoted_variable() {
    let out = run_shell("export PAIR='alpha beta'\necho $PAIR\nexit\n");
    assert!(out.contains("alpha beta"), "output was: {:?}", out);
}

#[test]
fn quoted_variable_keeps_spaces() {
    let out = run_shell("export SPACED='x y'\necho \"$SPACED\"\nexit\n");
    assert!(out.contains("x y"), "output was: {:?}", out);
}

#[test]
fn glob_matches_files() {
    std::fs::write("/tmp/eazyshell_globA.txt", "x").unwrap();
    std::fs::write("/tmp/eazyshell_globB.txt", "x").unwrap();
    std::fs::remove_file("/tmp/eazyshell_globC.txt").ok();
    let script = "/bin/echo /tmp/eazyshell_glob*.txt\nexit\n";
    let out = run_shell(script);
    assert!(out.contains("eazyshell_globA.txt"), "output was: {:?}", out);
    assert!(out.contains("eazyshell_globB.txt"), "output was: {:?}", out);
    std::fs::remove_file("/tmp/eazyshell_globA.txt").ok();
    std::fs::remove_file("/tmp/eazyshell_globB.txt").ok();
}

#[test]
fn if_then_else_runs_taken_branch() {
    let out = run_shell(
        "if /bin/true; then echo IFYES; else echo IFNO; fi\nif /bin/false; then echo Y2; else echo E2; fi\nexit\n",
    );
    assert!(out.contains("IFYES"), "output was: {:?}", out);
    assert!(out.contains("E2"), "output was: {:?}", out);
    assert!(!out.contains("IFNO"), "output was: {:?}", out);
    assert!(!out.contains("Y2"), "output was: {:?}", out);
}

#[test]
fn if_elif_chain() {
    let out = run_shell(
        "if /bin/false; then echo A; elif /bin/true; then echo ELIFOK; else echo B; fi\nexit\n",
    );
    assert!(out.contains("ELIFOK"), "output was: {:?}", out);
    assert!(!out.contains("A"), "output was: {:?}", out);
}

#[test]
fn while_loop_with_break() {
    let out = run_shell("i=0; while [ $i -lt 3 ]; do echo WH=$i; i=$((i+1)); done\nexit\n");
    assert!(out.contains("WH=0"), "output was: {:?}", out);
    assert!(out.contains("WH=1"), "output was: {:?}", out);
    assert!(out.contains("WH=2"), "output was: {:?}", out);
}

#[test]
fn for_loop_iterates_wordlist() {
    let out = run_shell("for v in alpha beta gamma; do echo LOOP=$v; done\nexit\n");
    assert!(out.contains("LOOP=alpha"), "output was: {:?}", out);
    assert!(out.contains("LOOP=beta"), "output was: {:?}", out);
    assert!(out.contains("LOOP=gamma"), "output was: {:?}", out);
}

#[test]
fn command_substitution_captures_output() {
    let out = run_shell("echo SUB $(/bin/echo captured)\nexit\n");
    assert!(out.contains("captured"), "output was: {:?}", out);
}

#[test]
fn command_substitution_inline_concatenation() {
    let out = run_shell("echo pre$(/bin/echo mid)post\nexit\n");
    assert!(out.contains("premidpost"), "output was: {:?}", out);
}

#[test]
fn command_substitution_leading_inline_suffix() {
    let out = run_shell("echo $(/bin/echo hi)-world\nexit\n");
    assert!(out.contains("hi-world"), "output was: {:?}", out);
}

#[test]
fn command_substitution_leading_inline_with_variable() {
    let out = run_shell("g=hello; echo $(echo $g)-world\nexit\n");
    assert!(out.contains("hello-world"), "output was: {:?}", out);
}

#[test]
fn arithmetic_inline_suffix() {
    let out = run_shell("echo val$((2+3))x\nexit\n");
    assert!(out.contains("val5x"), "output was: {:?}", out);
}

#[test]
fn nested_command_substitution() {
    let out = run_shell("echo N $(echo OUT $(echo IN))\nexit\n");
    assert!(out.contains("IN"), "output was: {:?}", out);
    assert!(out.contains("OUT"), "output was: {:?}", out);
}

#[test]
fn background_job_sets_bang_and_shows_in_jobs() {
    let out = run_shell("/bin/sleep 1 &\necho BGPID=$!\njobs\nfg\necho after-fg\nexit\n");
    assert!(out.contains("BGPID="), "output was: {:?}", out);
    assert!(out.contains("[1]"), "output was: {:?}", out);
    assert!(out.contains("after-fg"), "output was: {:?}", out);
}

#[test]
fn background_only_last_segment() {
    let out = run_shell("echo bgstart; /bin/sleep 1 &\necho bgend\nexit\n");
    assert!(out.contains("bgstart"), "output was: {:?}", out);
    assert!(out.contains("bgend"), "output was: {:?}", out);
}

#[test]
fn double_quoted_argument_keeps_spaces() {
    let out = run_shell("echo \"hello world\"\nexit\n");
    assert!(out.contains("hello world"), "output was: {:?}", out);
}

#[test]
fn double_quoted_assignment_value() {
    let out = run_shell("x=\"a b c\"; echo X=$x\nexit\n");
    assert!(out.contains("X=a b c"), "output was: {:?}", out);
}

#[test]
fn unquoted_assignment_initializes_variable() {
    let out = run_shell("x=initial; echo X=$x\nexit\n");
    assert!(out.contains("X=initial"), "output was: {:?}", out);
}

#[test]
fn command_substitution_with_quoted_inner_word() {
    let out = run_shell("echo $(/bin/echo \"a b c\")\nexit\n");
    assert!(out.contains("a b c"), "output was: {:?}", out);
}

#[test]
fn command_substitution_inline_preserves_inner_spaces() {
    let out = run_shell("echo P=$(/bin/echo \"x y z\")Q\nexit\n");
    assert!(out.contains("P=x y zQ"), "output was: {:?}", out);
}

#[test]
fn command_substitution_uses_shell_variables() {
    let out = run_shell("g=hello; echo $(echo $g)-world\nexit\n");
    assert!(out.contains("hello-world"), "output was: {:?}", out);
}

#[test]
fn command_substitution_empty_concatenates() {
    let out = run_shell("echo A$()B\nexit\n");
    assert!(out.contains("AB"), "output was: {:?}", out);
}

#[test]
fn command_substitution_with_arithmetic() {
    let out = run_shell("echo N=$(echo $((3+4)))M\nexit\n");
    assert!(out.contains("N=7M"), "output was: {:?}", out);
}

#[test]
fn command_substitution_in_double_quotes() {
    let out = run_shell("echo \"$(/bin/echo inside)\"\nexit\n");
    assert!(out.contains("inside"), "output was: {:?}", out);
}

#[test]
fn cd_and_pwd_builtins() {
    let out = run_shell("cd /tmp; pwd\nexit\n");
    assert!(out.contains("/tmp"), "output was: {:?}", out);
}

#[test]
fn last_status_special_variable() {
    let out = run_shell("/bin/false; echo STATUS=$?\nexit\n");
    assert!(out.contains("STATUS=1"), "output was: {:?}", out);
}

#[test]
fn if_without_else_continues() {
    let out = run_shell("if /bin/true; then echo TAKEN; fi; echo DONE\nexit\n");
    assert!(out.contains("TAKEN"), "output was: {:?}", out);
    assert!(out.contains("DONE"), "output was: {:?}", out);
}

#[test]
fn elif_chain_with_else() {
    let out = run_shell(
        "x=5; if [ $x -eq 9 ]; then echo NINE; elif [ $x -eq 5 ]; then echo FIVE; else echo OTHER; fi\nexit\n",
    );
    assert!(out.contains("FIVE"), "output was: {:?}", out);
    assert!(!out.contains("NINE"), "output was: {:?}", out);
    assert!(!out.contains("OTHER"), "output was: {:?}", out);
}

#[test]
fn nested_loop_with_conditional() {
    let out = run_shell("for i in 1 2 3; do if [ $i -ne 2 ]; then echo N$i; fi; done\nexit\n");
    assert!(out.contains("N1"), "output was: {:?}", out);
    assert!(out.contains("N3"), "output was: {:?}", out);
    assert!(!out.contains("N2"), "output was: {:?}", out);
}

#[test]
fn while_false_runs_zero_times() {
    let out = run_shell("i=0; while [ $i -gt 5 ]; do echo NO; done; echo AFTER\nexit\n");
    assert!(!out.contains("NO"), "output was: {:?}", out);
    assert!(out.contains("AFTER"), "output was: {:?}", out);
}

#[test]
fn group_command_runs_in_current_shell() {
    let out = run_shell("( echo grouped-in )\nexit\n");
    assert!(out.contains("grouped-in"), "output was: {:?}", out);
}

#[test]
fn background_command_with_assignment() {
    let out = run_shell("M=bgval /bin/echo bg-ran &\necho after\nexit\n");
    assert!(out.contains("after"), "output was: {:?}", out);
}

#[test]
fn double_quoted_variable_keeps_single_field() {
    let out = run_shell("v=\"a b\"; echo \"[$v]\"\nexit\n");
    assert!(out.contains("[a b]"), "output was: {:?}", out);
}

#[test]
fn empty_argv_pipeline_does_not_panic() {
    let out = run_shell("VAR=x | $UNDEFINED_NONE\necho shell-alive\nexit\n");
    assert!(!out.contains("panicked"), "output was: {:?}", out);
    assert!(out.contains("shell-alive"), "output was: {:?}", out);
}

#[test]
fn empty_var_in_pipeline_does_not_panic() {
    let out = run_shell("echo a | $UNSET_VAR | echo c\necho alive\nexit\n");
    assert!(!out.contains("panicked"), "output was: {:?}", out);
    assert!(out.contains("alive"), "output was: {:?}", out);
}

#[test]
fn undefined_var_background_does_not_panic() {
    let out = run_shell("$UNDEF_BG &\necho bg-alive\nexit\n");
    assert!(!out.contains("panicked"), "output was: {:?}", out);
    assert!(out.contains("bg-alive"), "output was: {:?}", out);
}

#[test]
fn undefined_var_alone_is_graceful() {
    let out = run_shell("echo $UNSET_SOLO\necho still-here\nexit\n");
    assert!(!out.contains("panicked"), "output was: {:?}", out);
    assert!(out.contains("still-here"), "output was: {:?}", out);
}

#[test]
fn bracket_test_command_name_not_mangled_by_glob() {
    let out = run_shell("[ 0 -eq 1 ]; echo rc=$?\nexit\n");
    assert!(!out.contains("failed to launch"), "output was: {:?}", out);
    assert!(out.contains("rc=1"), "output was: {:?}", out);
}

#[test]
fn bracket_test_conditionals_in_loop() {
    let out = run_shell("i=0; while [ $i -lt 3 ]; do echo IT=$i; i=$((i+1)); done\nexit\n");
    assert!(out.contains("IT=0"), "output was: {:?}", out);
    assert!(out.contains("IT=1"), "output was: {:?}", out);
    assert!(out.contains("IT=2"), "output was: {:?}", out);
    assert!(!out.contains("failed to launch"), "output was: {:?}", out);
}

#[test]
fn exit_code_with_argument_is_propagated() {
    assert_eq!(shell_exit_code("exit 3\n"), 3);
    assert_eq!(shell_exit_code("exit 0\n"), 0);
}

#[test]
fn exit_code_reflects_last_status() {
    assert_eq!(shell_exit_code("/bin/false\nexit $?\n"), 1);
    assert_eq!(shell_exit_code("/bin/true\nexit $?\n"), 0);
    assert_eq!(shell_exit_code("/bin/false\n exit\n"), 1);
}

#[test]
fn completed_background_job_is_reaped_and_listed() {
    let out = run_shell("/bin/true &\n/bin/sleep 0.3\njobs\nexit\n");
    assert!(out.contains("[1]"), "output was: {:?}", out);
    assert!(out.contains("Done"), "output was: {:?}", out);
    assert!(!out.contains("Running"), "output was: {:?}", out);
}

#[test]
fn fg_with_no_jobs_errors_gracefully() {
    let out = run_shell("fg\necho fg-rc=$?\nexit\n");
    assert!(out.contains("fg-rc=1"), "output was: {:?}", out);
    assert!(!out.contains("panicked"), "output was: {:?}", out);
}

#[test]
fn background_job_with_failed_command_is_reaped() {
    let out = run_shell("/bin/false &\n/bin/sleep 0.3\njobs\nexit\n");
    assert!(out.contains("Done(1)"), "output was: {:?}", out);
}
