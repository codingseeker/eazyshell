use std::collections::VecDeque;
use std::process::Child;

pub enum JobStatus {
    Running,
    Done(i32),
}

pub struct Job {
    pub id: usize,
    pub child: Child,
    pub cmdline: String,
    pub status: JobStatus,
}

pub struct Shell {
    pub variables: Vec<(String, String)>,
    pub aliases: Vec<(String, String)>,
    pub last_status: i32,
    pub oldpwd: Option<String>,
    pub running: bool,
    pub pending_heredocs: VecDeque<String>,
    pub jobs: Vec<Job>,
    pub next_job_id: usize,
    pub last_bg_pid: Option<i32>,
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            variables: std::env::vars().collect(),
            aliases: Vec::new(),
            last_status: 0,
            oldpwd: None,
            running: true,
            pending_heredocs: VecDeque::new(),
            jobs: Vec::new(),
            next_job_id: 1,
            last_bg_pid: None,
        }
    }

    pub fn add_job(&mut self, child: Child, cmdline: String) -> usize {
        let id = self.next_job_id;
        self.next_job_id += 1;
        self.jobs.push(Job {
            id,
            child,
            cmdline,
            status: JobStatus::Running,
        });
        id
    }

    pub fn reap_jobs(&mut self) {
        self.jobs.retain_mut(|job| {
            if let JobStatus::Running = job.status {
                match job.child.try_wait() {
                    Ok(Some(st)) => {
                        job.status = JobStatus::Done(status_code(st));
                        true
                    }
                    Ok(None) => true,
                    Err(_) => {
                        job.status = JobStatus::Done(-1);
                        true
                    }
                }
            } else {
                true
            }
        });
    }

    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.variables
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn set_var(&mut self, name: &str, value: &str) {
        match self.variables.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value.to_string(),
            None => self.variables.push((name.to_string(), value.to_string())),
        }
    }

    pub fn unset_var(&mut self, name: &str) -> bool {
        let before = self.variables.len();
        self.variables.retain(|(n, _)| n != name);
        self.variables.len() != before
    }

    pub fn get_alias(&self, name: &str) -> Option<&str> {
        self.aliases
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn set_alias(&mut self, name: &str, value: &str) {
        match self.aliases.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value.to_string(),
            None => self.aliases.push((name.to_string(), value.to_string())),
        }
    }

    pub fn home(&self) -> Option<String> {
        self.get_var("HOME").map(|s| s.to_string())
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

pub fn status_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    if let Some(code) = status.code() {
        code
    } else if let Some(sig) = status.signal() {
        128 + sig
    } else {
        1
    }
}
