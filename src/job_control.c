#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <unistd.h>
#include "job_control.h"

Job jobs[MAX_JOBS];
int job_count = 0;
int next_job_id = 1;
pid_t shell_pgid;
int shell_terminal;

void add_job(pid_t pgid, char *cmdline, int fg) {
    if (job_count >= MAX_JOBS) return;
    Job *j = &jobs[job_count++];
    j->job_id = next_job_id++;
    j->pgid = pgid;
    j->cmdline = strdup(cmdline);
    j->state = 0;
    j->foreground = fg;
    if (fg) tcsetpgrp(shell_terminal, pgid);
}

void remove_job(pid_t pgid) {
    for (int i = 0; i < job_count; i++) {
        if (jobs[i].pgid == pgid) {
            free(jobs[i].cmdline);
            jobs[i] = jobs[--job_count];
            break;
        }
    }
}

Job *find_job_by_id(int id) {
    for (int i = 0; i < job_count; i++)
        if (jobs[i].job_id == id) return &jobs[i];
    return NULL;
}

static void sigchld_handler(int sig) {
    pid_t pid;
    int status;
    while ((pid = waitpid(-1, &status, WNOHANG | WUNTRACED)) > 0) {
        if (getpgid(pid) == -1) continue;
        for (int i = 0; i < job_count; i++) {
            if (getpgid(pid) == jobs[i].pgid) {
                if (WIFSTOPPED(status)) jobs[i].state = 1;
                else remove_job(jobs[i].pgid);
                break;
            }
        }
    }
}

void setup_signals(void) {
    shell_terminal = STDIN_FILENO;
    while (tcgetpgrp(shell_terminal) != (shell_pgid = getpgrp()))
        kill(-shell_pgid, SIGTTIN);
    struct sigaction sa = { .sa_handler = SIG_IGN };
    sigemptyset(&sa.sa_mask);
    sigaction(SIGINT, &sa, NULL);
    sigaction(SIGQUIT, &sa, NULL);
    sigaction(SIGTSTP, &sa, NULL);
    sigaction(SIGTTIN, &sa, NULL);
    sigaction(SIGTTOU, &sa, NULL);
    sa.sa_handler = sigchld_handler;
    sa.sa_flags = SA_RESTART;
    sigaction(SIGCHLD, &sa, NULL);
}