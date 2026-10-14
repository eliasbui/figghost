//! Optional timeline for debugging focus and window problems: `FIGGHOST_TRACE=1` prints epoch
//! milliseconds and a short note to stderr.

use crate::history::now_ms;
use std::env;

pub fn enabled() -> bool {
    env::var_os("FIGGHOST_TRACE").is_some()
}

pub fn log(what: &str) {
    if enabled() {
        eprintln!("{} {what}", now_ms());
    }
}
