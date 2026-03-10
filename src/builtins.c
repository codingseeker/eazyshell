#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "builtins.h"
#include "job_control.h"

#define MAX_ALIASES 64

Alias aliases[MAX_ALIASES];
int alias_count = 0;

int is_builtin_name(const char *name) {
    if (!name) return 0;
    return (strcmp(name, "cd") == 0 ||
            strcmp(name, "exit") == 0 ||
            strcmp(name, "jobs") == 0 ||
            strcmp(name, "fg") == 0 ||
            strcmp(name, "bg") == 0 ||
            strcmp(name, "pwd") == 0 ||
            strcmp(name, "export") == 0 ||
            strcmp(name, "unset") == 0 ||
            strcmp(name, "echo") == 0 ||
            strcmp(name, "alias") == 0 ||
            strcmp(name, "type") == 0);
}

int is_builtin(Command *cmd) {
    if (!cmd->argv[0]) return 0;
    return is_builtin_name(cmd->argv[0]);
}

int do_cd(Command *cmd) {
    char *dir = cmd->argv[1];
    static char *oldpwd = NULL;
    if (!dir || strcmp(dir, "~") == 0) dir = getenv("HOME");
    else if (strcmp(dir, "-") == 0) dir = oldpwd;
    if (!dir) return 1;
    char *cur = get_current_dir_name();
    if (!cur) { perror("get_current_dir_name"); return 1; }
    if (chdir(dir) == 0) {
        if (oldpwd) free(oldpwd);
        oldpwd = cur;
        return 0;
    }
    perror("cd");
    free(cur);
    return 1;
}

int do_pwd(Command *cmd) {
    char *cwd = get_current_dir_name();
    if (cwd) {
        printf("%s\n", cwd);
        free(cwd);
        return 0;
    }
    perror("pwd");
    return 1;
}

int do_export(Command *cmd) {
    if (!cmd->argv[1]) {
        extern char **environ;
        for (char **e = environ; *e; e++) printf("%s\n", *e);
        return 0;
    }
    for (int i = 1; cmd->argv[i]; i++) {
        char *eq = strchr(cmd->argv[i], '=');
        if (eq) {
            *eq = '\0';
            setenv(cmd->argv[i], eq+1, 1);
        } else {
            setenv(cmd->argv[i], "", 1);
        }
    }
    return 0;
}

int do_unset(Command *cmd) {
    for (int i = 1; cmd->argv[i]; i++) unsetenv(cmd->argv[i]);
    return 0;
}

int do_echo(Command *cmd) {
    int nflag = 0;
    int start = 1;
    if (cmd->argv[1] && strcmp(cmd->argv[1], "-n") == 0) {
        nflag = 1;
        start = 2;
    }
    for (int i = start; cmd->argv[i]; i++) {
        printf("%s", cmd->argv[i]);
        if (cmd->argv[i+1]) printf(" ");
    }
    if (!nflag) printf("\n");
    return 0;
}

int do_alias(Command *cmd) {
    if (!cmd->argv[1]) {
        for (int i = 0; i < alias_count; i++) printf("alias %s='%s'\n", aliases[i].name, aliases[i].value);
        return 0;
    }
    for (int i = 1; cmd->argv[i]; i++) {
        char *eq = strchr(cmd->argv[i], '=');
        if (eq) {
            *eq = '\0';
            char *name = cmd->argv[i];
            char *value = eq+1;
            if (alias_count < MAX_ALIASES) {
                aliases[alias_count].name = strdup(name);
                aliases[alias_count].value = strdup(value);
                alias_count++;
            }
        } else {
            for (int j = 0; j < alias_count; j++) {
                if (strcmp(aliases[j].name, cmd->argv[i]) == 0) {
                    printf("alias %s='%s'\n", aliases[j].name, aliases[j].value);
                    break;
                }
            }
        }
    }
    return 0;
}

int do_type(Command *cmd) {
    if (!cmd->argv[1]) return 0;
    char *arg = cmd->argv[1];
    if (is_builtin_name(arg)) {
        printf("%s is a shell builtin\n", arg);
        return 0;
    }
    for (int i = 0; i < alias_count; i++) {
        if (strcmp(aliases[i].name, arg) == 0) {
            printf("%s is aliased to '%s'\n", arg, aliases[i].value);
            return 0;
        }
    }
    char *path = getenv("PATH");
    if (!path) return 1;
    char *path_copy = strdup(path);
    char *dir = strtok(path_copy, ":");
    while (dir) {
        char full[1024];
        snprintf(full, sizeof(full), "%s/%s", dir, arg);
        if (access(full, X_OK) == 0) {
            printf("%s is %s\n", arg, full);
            free(path_copy);
            return 0;
        }
        dir = strtok(NULL, ":");
    }
    free(path_copy);
    printf("%s not found\n", arg);
    return 1;
}

int do_fg(Command *cmd) {
    int id = cmd->argv[1] ? atoi(cmd->argv[1]) : (job_count ? jobs[job_count-1].job_id : -1);
    Job *j = find_job_by_id(id);
    if (!j) { fprintf(stderr, "fg: job not found\n"); return 1; }
    j->foreground = 1;
    j->state = 0;
    tcsetpgrp(shell_terminal, j->pgid);
    kill(-j->pgid, SIGCONT);
    int status;
    waitpid(-j->pgid, &status, WUNTRACED);
    if (WIFSTOPPED(status)) j->state = 1;
    else remove_job(j->pgid);
    tcsetpgrp(shell_terminal, shell_pgid);
    return 0;
}

int do_bg(Command *cmd) {
    int id = cmd->argv[1] ? atoi(cmd->argv[1]) : (job_count ? jobs[job_count-1].job_id : -1);
    Job *j = find_job_by_id(id);
    if (!j) { fprintf(stderr, "bg: job not found\n"); return 1; }
    j->foreground = 0;
    j->state = 0;
    kill(-j->pgid, SIGCONT);
    return 0;
}

int do_jobs(void) {
    for (int i = 0; i < job_count; i++) {
        printf("[%d] %c %s\n", jobs[i].job_id,
               jobs[i].state == 0 ? 'R' : 'T', jobs[i].cmdline);
    }
    return 0;
}