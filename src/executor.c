#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include "executor.h"
#include "builtins.h"
#include "job_control.h"

#define MAX_CMDS 16

int exec_pipeline(Pipeline *p, int bg, int subshell) {
    int prev_pipe[2] = {-1, -1};
    pid_t pids[MAX_CMDS];
    pid_t pgid = 0;
    for (int i = 0; i < p->count; i++) {
        int pipefd[2] = {-1, -1};
        if (i < p->count - 1) {
            if (pipe(pipefd) < 0) { perror("pipe"); return 1; }
        }
        pid_t pid = fork();
        if (pid < 0) { perror("fork"); return 1; }
        if (pid == 0) {
            if (!subshell) {
                struct sigaction sa = { .sa_handler = SIG_DFL };
                sigaction(SIGINT, &sa, NULL);
                sigaction(SIGQUIT, &sa, NULL);
                sigaction(SIGTSTP, &sa, NULL);
            }
            if (prev_pipe[0] != -1) {
                dup2(prev_pipe[0], STDIN_FILENO);
                close(prev_pipe[0]); close(prev_pipe[1]);
            }
            if (pipefd[1] != -1) {
                dup2(pipefd[1], STDOUT_FILENO);
                close(pipefd[0]); close(pipefd[1]);
            }
            if (p->cmds[i]->heredoc_fd != -1) {
                dup2(p->cmds[i]->heredoc_fd, STDIN_FILENO);
                close(p->cmds[i]->heredoc_fd);
            } else {
                if (p->cmds[i]->infile) {
                    int fd = open(p->cmds[i]->infile, O_RDONLY);
                    if (fd >= 0) { dup2(fd, STDIN_FILENO); close(fd); }
                }
            }
            if (p->cmds[i]->outfile) {
                int fd = open(p->cmds[i]->outfile, O_WRONLY|O_CREAT|O_TRUNC, 0644);
                if (fd >= 0) { dup2(fd, STDOUT_FILENO); close(fd); }
            }
            if (!subshell) {
                if (pgid == 0) pgid = getpid();
                setpgid(0, pgid);
                if (!bg && i == 0) tcsetpgrp(shell_terminal, pgid);
            }
            execvp(p->cmds[i]->argv[0], p->cmds[i]->argv);
            perror("execvp"); exit(1);
        }
        pids[i] = pid;
        if (!subshell) {
            if (pgid == 0) pgid = pid;
            setpgid(pid, pgid);
        }
        if (prev_pipe[0] != -1) { close(prev_pipe[0]); close(prev_pipe[1]); }
        prev_pipe[0] = pipefd[0];
        prev_pipe[1] = pipefd[1];
        if (pipefd[1] != -1) close(pipefd[1]);
        if (p->cmds[i]->heredoc_fd != -1) {
            close(p->cmds[i]->heredoc_fd);
            p->cmds[i]->heredoc_fd = -1;
        }
    }
    if (subshell) {
        for (int i = 0; i < p->count; i++) waitpid(pids[i], NULL, 0);
        return 0;
    }
    if (!bg) {
        tcsetpgrp(shell_terminal, pgid);
        int status;
        for (int i = 0; i < p->count; i++) waitpid(pids[i], &status, WUNTRACED);
        if (WIFSTOPPED(status)) {
            for (int i = 0; i < job_count; i++)
                if (jobs[i].pgid == pgid) jobs[i].state = 1;
        } else remove_job(pgid);
        tcsetpgrp(shell_terminal, shell_pgid);
        return WEXITSTATUS(status);
    } else {
        char *cmdline = p->cmds[0]->argv[0];
        add_job(pgid, cmdline, 0);
        printf("[%d] %d\n", jobs[job_count-1].job_id, pgid);
        return 0;
    }
}

int eval_ast(ASTNode *node, int bg, int subshell) {
    if (!node) return 0;
    switch (node->type) {
        case NODE_COMMAND: {
            Command *cmd = node->data.cmd;
            if (is_builtin(cmd)) {
                if (strcmp(cmd->argv[0], "cd") == 0) return do_cd(cmd);
                if (strcmp(cmd->argv[0], "exit") == 0) exit(0);
                if (strcmp(cmd->argv[0], "jobs") == 0) return do_jobs();
                if (strcmp(cmd->argv[0], "fg") == 0) return do_fg(cmd);
                if (strcmp(cmd->argv[0], "bg") == 0) return do_bg(cmd);
                if (strcmp(cmd->argv[0], "pwd") == 0) return do_pwd(cmd);
                if (strcmp(cmd->argv[0], "export") == 0) return do_export(cmd);
                if (strcmp(cmd->argv[0], "unset") == 0) return do_unset(cmd);
                if (strcmp(cmd->argv[0], "echo") == 0) return do_echo(cmd);
                if (strcmp(cmd->argv[0], "alias") == 0) return do_alias(cmd);
                if (strcmp(cmd->argv[0], "type") == 0) return do_type(cmd);
            }
            Pipeline *p = malloc(sizeof(Pipeline));
            p->cmds = malloc(sizeof(Command*));
            p->cmds[0] = cmd;
            p->count = 1;
            int ret = exec_pipeline(p, bg, subshell);
            free(p->cmds);
            free(p);
            return ret;
        }
        case NODE_PIPELINE:
            return exec_pipeline(node->data.pipeline, bg, subshell);
        case NODE_AND: {
            int l = eval_ast(node->data.binary.left, 0, subshell);
            if (l == 0) return eval_ast(node->data.binary.right, 0, subshell);
            return l;
        }
        case NODE_OR: {
            int l = eval_ast(node->data.binary.left, 0, subshell);
            if (l != 0) return eval_ast(node->data.binary.right, 0, subshell);
            return l;
        }
        case NODE_SEMICOLON:
            eval_ast(node->data.binary.left, 0, subshell);
            return eval_ast(node->data.binary.right, 0, subshell);
        case NODE_IF: {
            int cond = eval_ast(node->data.if_stmt.condition, 0, subshell);
            if (cond == 0)
                return eval_ast(node->data.if_stmt.then_body, 0, subshell);
            else if (node->data.if_stmt.else_body)
                return eval_ast(node->data.if_stmt.else_body, 0, subshell);
            return cond;
        }
        case NODE_WHILE: {
            int last = 0;
            while (eval_ast(node->data.while_stmt.condition, 0, subshell) == 0) {
                last = eval_ast(node->data.while_stmt.body, 0, subshell);
            }
            return last;
        }
        case NODE_FOR: {
            int last = 0;
            for (int i = 0; i < node->data.for_stmt.wordcount; i++) {
                setenv(node->data.for_stmt.var, node->data.for_stmt.wordlist[i], 1);
                last = eval_ast(node->data.for_stmt.body, 0, subshell);
            }
            return last;
        }
        case NODE_GROUP: {
            pid_t pid = fork();
            if (pid == 0) {
                eval_ast(node->data.group.body, 0, 1);
                exit(0);
            } else {
                int status;
                waitpid(pid, &status, 0);
                return WEXITSTATUS(status);
            }
        }
    }
    return 0;
}