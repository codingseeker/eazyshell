#ifndef JOB_CONTROL_H
#define JOB_CONTROL_H

#include <sys/types.h>

#define MAX_JOBS 64

typedef struct Job {
    int job_id;
    pid_t pgid;
    char *cmdline;
    int state;          /* 0 running, 1 stopped */
    int foreground;
} Job;

extern Job jobs[MAX_JOBS];
extern int job_count;
extern int next_job_id;
extern pid_t shell_pgid;
extern int shell_terminal;

void add_job(pid_t pgid, char *cmdline, int fg);
void remove_job(pid_t pgid);
Job *find_job_by_id(int id);
void setup_signals(void);

#endif