//! Stopping what a command started.

/// Kills every process of the process group `group`, which a command made its own with `process_group(0)` so that what it
/// starts can be stopped with it. A group that is already gone is no error, and the numbers 0 and 1, which would reach
/// this app's own group or every process there is, are never signalled.
pub fn kill_group(group: u32) {
    let Ok(group) = libc::pid_t::try_from(group) else {
        return;
    };
    if group > 1 {
        // SAFETY: a plain system call with a process group number and a signal number; it touches no memory of ours.
        unsafe { libc::killpg(group, libc::SIGKILL) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    #[test]
    fn a_group_is_stopped_with_what_it_started_and_the_numbers_that_would_do_harm_are_ignored() {
        let mut shell = Command::new("/bin/sh")
            .args(["-c", "sleep 32 & sleep 32"])
            .stdout(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        // These must do nothing at all, or this test would take the whole test run with it.
        kill_group(0);
        kill_group(1);
        kill_group(u32::MAX);
        assert!(shell.try_wait().unwrap().is_none(), "it is still running");
        kill_group(shell.id());
        let started = Instant::now();
        let status = shell.wait().unwrap();
        assert!(!status.success(), "it was killed");
        assert!(started.elapsed() < Duration::from_secs(5));
        let sleeping = Command::new("/usr/bin/pgrep")
            .args(["-f", "sleep 32"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false);
        let mut gone = !sleeping;
        for _ in 0..20 {
            if gone {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
            gone = !Command::new("/usr/bin/pgrep")
                .args(["-f", "sleep 32"])
                .output()
                .is_ok_and(|out| out.status.success());
        }
        assert!(gone, "what it started was stopped too");
    }
}
