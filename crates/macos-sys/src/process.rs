//! Running a command with a time limit, and stopping what a command started.

use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Runs `command` and returns what it printed, or `None` when it fails or is still running after `timeout`, in which
/// case it is killed. Blocks the calling thread, so it belongs off the main one.
pub fn run_capture(command: &mut Command, timeout: Duration) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => {
                let mut out = String::new();
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                return Some(out);
            }
            Some(_) => return None,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(Duration::from_millis(15)),
        }
    }
}

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
    fn a_command_gives_what_it_printed_and_one_that_runs_too_long_is_stopped() {
        assert_eq!(
            run_capture(
                Command::new("/bin/echo").arg("hello"),
                Duration::from_secs(5)
            )
            .as_deref(),
            Some("hello\n")
        );
        assert!(run_capture(&mut Command::new("/usr/bin/false"), Duration::from_secs(5)).is_none());
        let started = Instant::now();
        assert!(
            run_capture(
                Command::new("/bin/sleep").arg("30"),
                Duration::from_millis(200)
            )
            .is_none()
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

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
