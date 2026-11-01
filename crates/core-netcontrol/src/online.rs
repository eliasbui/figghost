//! Whether the Mac reaches the internet: what the probes found over time, turned into the moments worth telling. Pure,
//! so every rule is a unit test; the probing itself is in the app.

/// What one probe found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// A connection was made; `captive` when the page that must say `Success` said something else, as a network that
    /// asks for a login does.
    Ok {
        captive: bool,
    },
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Unknown,
    Online,
    Offline,
    Captive,
}

/// What is worth telling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    WentOffline,
    BackOnline,
    NeedsLogin,
}

/// A probe that fails once is a hiccup; this many in a row is no internet.
pub const FAILURES_TO_GO_OFFLINE: u32 = 2;

#[derive(Debug, Default)]
pub struct Tracker {
    state: State,
    failures: u32,
}

impl Tracker {
    pub fn state(&self) -> State {
        self.state
    }

    /// Takes one probe and says what happened, if anything. What the Mac starts in is not announced: only a change is.
    pub fn step(&mut self, reach: Reach) -> Option<Event> {
        let before = self.state;
        match reach {
            Reach::Fail => {
                self.failures += 1;
                if self.failures >= FAILURES_TO_GO_OFFLINE {
                    self.state = State::Offline;
                }
            }
            Reach::Ok { captive } => {
                self.failures = 0;
                self.state = if captive {
                    State::Captive
                } else {
                    State::Online
                };
            }
        }
        match (before, self.state) {
            (State::Online | State::Captive, State::Offline) => Some(Event::WentOffline),
            (State::Offline | State::Captive, State::Online) => Some(Event::BackOnline),
            (State::Unknown | State::Online | State::Offline, State::Captive) => {
                Some(Event::NeedsLogin)
            }
            _ => None,
        }
    }
}

/// Whether the page of the check says what the real one says.
pub fn is_success(body: &str) -> bool {
    body.contains("Success")
}

/// What a check that got a page says: a page that is empty says nothing, so it does not call the network captive.
pub fn captive_from(body: &str) -> bool {
    !body.trim().is_empty() && !is_success(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: Reach = Reach::Ok { captive: false };
    const LOGIN: Reach = Reach::Ok { captive: true };

    #[test]
    fn the_start_is_not_announced_and_one_failure_is_a_hiccup() {
        let mut t = Tracker::default();
        assert_eq!(t.step(OK), None);
        assert_eq!(t.state(), State::Online);
        assert_eq!(t.step(Reach::Fail), None, "once is nothing");
        assert_eq!(t.state(), State::Online);
        assert_eq!(t.step(OK), None);
        assert_eq!(t.step(Reach::Fail), None, "the count started again");
    }

    #[test]
    fn two_failures_in_a_row_are_told_once_and_the_return_is_told() {
        let mut t = Tracker::default();
        t.step(OK);
        t.step(Reach::Fail);
        assert_eq!(t.step(Reach::Fail), Some(Event::WentOffline));
        assert_eq!(t.step(Reach::Fail), None, "told once");
        assert_eq!(t.step(OK), Some(Event::BackOnline));
        assert_eq!(t.step(OK), None);
    }

    #[test]
    fn a_network_that_asks_for_a_login_is_told_and_so_is_the_login_done() {
        let mut t = Tracker::default();
        assert_eq!(
            t.step(LOGIN),
            Some(Event::NeedsLogin),
            "from the start too: this is what is worth knowing"
        );
        assert_eq!(t.step(LOGIN), None);
        assert_eq!(t.step(OK), Some(Event::BackOnline));
        assert_eq!(
            t.step(LOGIN),
            Some(Event::NeedsLogin),
            "a login asked for again"
        );
        t.step(Reach::Fail);
        assert_eq!(t.step(Reach::Fail), Some(Event::WentOffline));
        assert_eq!(
            t.step(LOGIN),
            Some(Event::NeedsLogin),
            "back, but to a page that wants a login"
        );
    }

    #[test]
    fn starting_offline_is_told_when_the_network_comes_back_and_not_before() {
        let mut t = Tracker::default();
        t.step(Reach::Fail);
        assert_eq!(
            t.step(Reach::Fail),
            None,
            "no one was told it was ever online"
        );
        assert_eq!(t.state(), State::Offline);
        assert_eq!(t.step(OK), Some(Event::BackOnline));
    }

    #[test]
    fn the_page_of_the_check_decides_whether_a_network_is_captive() {
        assert!(is_success(
            "<HTML><HEAD><TITLE>Success</TITLE></HEAD><BODY>Success</BODY></HTML>"
        ));
        assert!(!captive_from(
            "<HTML><HEAD><TITLE>Success</TITLE></HEAD><BODY>Success</BODY></HTML>"
        ));
        assert!(captive_from(
            "<html><body>Please sign in to the hotel Wi-Fi</body></html>"
        ));
        assert!(!captive_from(""), "an empty answer is no evidence");
        assert!(!captive_from("  \n"));
    }
}
