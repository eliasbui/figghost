//! What macOS reports as Now Playing (any app, browsers included), as one line of JSON per change on stdout.
//!
//! MediaRemote, the private framework behind the player in Control Center, only answers Apple's own processes on
//! current macOS. `/usr/bin/perl` is one of them and can load a library and call a function in it as an XSUB, so
//! the app runs
//!
//! `/usr/bin/perl -MDynaLoader -e '<load the file in $ARGV[0], install figghost_np_stream as np, call np()>' <dylib>`
//!
//! and reads the lines. `figghost_np_stream` never returns: it prints a line at start and one each time something
//! changes, and ends the process when stdout is closed or the app that started it is gone. This library is never
//! linked into the app.
//!
//! A line: `{"playing":true,"title":"...","artist":"...","album":"...","duration":2304.76,"elapsed":939.02,
//! "rate":1,"at_ms":1792049126808,"bundle":"com.microsoft.edgemac","parent":"","pid":16572}`, where `elapsed` is
//! the position at `at_ms` (milliseconds since 1970), numbers are `null` when unknown, and nothing playing is empty
//! strings with `"playing":false`. `"artwork":"<base64>","mime":"image/jpeg"` are added only when the picture
//! changed since the previous line, with empty strings when it went away.
//!
//! `figghost_np_seek`, called the same way with `FIGGHOST_NP_SEEK=<seconds>` in the environment, moves what plays to
//! that position and prints the line that follows.

use block2::{Block, RcBlock};
use dispatch2::DispatchQueue;
use objc2::{
    rc::{Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_foundation::{
    NSData, NSDate, NSDefaultRunLoopMode, NSDictionary, NSNotification, NSNotificationCenter,
    NSNumber, NSOperationQueue, NSRunLoop, NSString, NSTimer,
};
use std::{
    cell::{Cell, RefCell},
    collections::hash_map::DefaultHasher,
    ffi::{CStr, c_int, c_void},
    fmt::Write as _,
    hash::{Hash, Hasher},
    io::Write as _,
    ptr::NonNull,
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const FRAMEWORK: &CStr = c"/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote";
/// A bigger picture is left out rather than pushed through the pipe.
const MAX_ARTWORK: usize = 4 << 20;
/// A position this close to where the previous line said the player would be by now is not news.
const DRIFT_SECONDS: f64 = 1.5;
/// How often the stream checks that the app that started it still runs.
const PARENT_CHECK_SECONDS: f64 = 2.0;
/// How long `figghost_np_get` waits for MediaRemote.
const GET_TIMEOUT: Duration = Duration::from_secs(5);

type Register = unsafe extern "C" fn(&DispatchQueue);
type Ask<A> = unsafe extern "C" fn(&DispatchQueue, &Block<dyn Fn(A)>);
type InfoReply = *const NSDictionary<NSString, AnyObject>;
type ClientString = unsafe extern "C" fn(*mut AnyObject) -> *const NSString;

/// The keys of the Now Playing dictionary.
struct Keys {
    title: &'static NSString,
    artist: &'static NSString,
    album: &'static NSString,
    duration: &'static NSString,
    elapsed: &'static NSString,
    rate: &'static NSString,
    timestamp: &'static NSString,
    artwork: &'static NSString,
    mime: &'static NSString,
}

/// The parts of MediaRemote this uses, looked up at run time: the framework is private and has no headers.
struct MediaRemote {
    register: Register,
    info: Ask<InfoReply>,
    playing: Ask<u8>,
    pid: Ask<c_int>,
    client: Ask<*mut AnyObject>,
    client_bundle: ClientString,
    client_parent: ClientString,
    /// The notifications that say something changed: the track, playing or paused, the app.
    changes: [&'static NSString; 3],
    keys: Keys,
}

impl MediaRemote {
    /// `None` when the framework or one of the symbols is missing. Leaked: it lives as long as the process.
    fn load() -> Option<&'static Self> {
        // SAFETY: dlopen of a constant path; the handle is never closed, so every symbol stays valid.
        let lib = unsafe { libc::dlopen(FRAMEWORK.as_ptr(), libc::RTLD_LAZY) };
        if lib.is_null() {
            return None;
        }
        let find = |name: &CStr| {
            // SAFETY: `lib` is a live handle and the name is a C string.
            let found = unsafe { libc::dlsym(lib, name.as_ptr()) };
            (!found.is_null()).then_some(found)
        };
        let string = |name: &CStr| {
            let slot = find(name)?.cast::<*const NSString>();
            // SAFETY: the symbol is one of the framework's CFStringRef constants, which live as long as the process.
            unsafe { (*slot).as_ref() }
        };
        // SAFETY: each symbol is a function with exactly the C signature of the field it goes into, as every
        // MediaRemote client declares them; a data pointer and a function pointer have the same size here.
        let functions = unsafe {
            (
                std::mem::transmute::<*mut c_void, Register>(find(
                    c"MRMediaRemoteRegisterForNowPlayingNotifications",
                )?),
                std::mem::transmute::<*mut c_void, Ask<InfoReply>>(find(
                    c"MRMediaRemoteGetNowPlayingInfo",
                )?),
                std::mem::transmute::<*mut c_void, Ask<u8>>(find(
                    c"MRMediaRemoteGetNowPlayingApplicationIsPlaying",
                )?),
                std::mem::transmute::<*mut c_void, Ask<c_int>>(find(
                    c"MRMediaRemoteGetNowPlayingApplicationPID",
                )?),
                std::mem::transmute::<*mut c_void, Ask<*mut AnyObject>>(find(
                    c"MRMediaRemoteGetNowPlayingClient",
                )?),
                std::mem::transmute::<*mut c_void, ClientString>(find(
                    c"MRNowPlayingClientGetBundleIdentifier",
                )?),
                std::mem::transmute::<*mut c_void, ClientString>(find(
                    c"MRNowPlayingClientGetParentAppBundleIdentifier",
                )?),
            )
        };
        let (register, info, playing, pid, client, client_bundle, client_parent) = functions;
        let this = Self {
            register,
            info,
            playing,
            pid,
            client,
            client_bundle,
            client_parent,
            changes: [
                string(c"kMRMediaRemoteNowPlayingInfoDidChangeNotification")?,
                string(c"kMRMediaRemoteNowPlayingApplicationIsPlayingDidChangeNotification")?,
                string(c"kMRMediaRemoteNowPlayingApplicationDidChangeNotification")?,
            ],
            keys: Keys {
                title: string(c"kMRMediaRemoteNowPlayingInfoTitle")?,
                artist: string(c"kMRMediaRemoteNowPlayingInfoArtist")?,
                album: string(c"kMRMediaRemoteNowPlayingInfoAlbum")?,
                duration: string(c"kMRMediaRemoteNowPlayingInfoDuration")?,
                elapsed: string(c"kMRMediaRemoteNowPlayingInfoElapsedTime")?,
                rate: string(c"kMRMediaRemoteNowPlayingInfoPlaybackRate")?,
                timestamp: string(c"kMRMediaRemoteNowPlayingInfoTimestamp")?,
                artwork: string(c"kMRMediaRemoteNowPlayingInfoArtworkData")?,
                mime: string(c"kMRMediaRemoteNowPlayingInfoArtworkMIMEType")?,
            },
        };
        Some(Box::leak(Box::new(this)))
    }
}

/// The encoded picture with a hash of it, to tell when it changed.
struct Picture {
    id: u64,
    bytes: Vec<u8>,
    mime: String,
}

/// Everything one line says.
#[derive(Default)]
struct Now {
    playing: bool,
    title: String,
    artist: String,
    album: String,
    duration: Option<f64>,
    elapsed: Option<f64>,
    rate: Option<f64>,
    /// When `elapsed` was measured, in milliseconds since 1970.
    at_ms: f64,
    bundle: String,
    parent: String,
    pid: i32,
    artwork: Option<Picture>,
}

impl Now {
    /// Seconds of the track per second of time.
    fn speed(&self) -> f64 {
        if self.playing {
            self.rate.unwrap_or(1.0)
        } else {
            0.0
        }
    }

    fn picture_id(&self) -> Option<u64> {
        self.artwork.as_ref().map(|p| p.id)
    }
}

fn wall_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn read_info(keys: &Keys, info: &NSDictionary<NSString, AnyObject>, now: &mut Now) {
    let text = |key: &NSString| {
        info.objectForKey(key)
            .and_then(|v| v.downcast::<NSString>().ok())
            .map_or_else(String::new, |s| s.to_string())
    };
    let number = |key: &NSString| {
        info.objectForKey(key)
            .and_then(|v| v.downcast::<NSNumber>().ok())
            .map(|n| n.doubleValue())
            .filter(|n| n.is_finite())
    };
    now.title = text(keys.title);
    now.artist = text(keys.artist);
    now.album = text(keys.album);
    now.duration = number(keys.duration);
    now.elapsed = number(keys.elapsed);
    now.rate = number(keys.rate);
    if let Some(date) = info
        .objectForKey(keys.timestamp)
        .and_then(|v| v.downcast::<NSDate>().ok())
    {
        let at = date.timeIntervalSince1970() * 1000.0;
        if at.is_finite() && at > 0.0 {
            now.at_ms = at;
        }
    }
    now.artwork = info
        .objectForKey(keys.artwork)
        .and_then(|v| v.downcast::<NSData>().ok())
        .filter(|data| (1..=MAX_ARTWORK).contains(&data.length()))
        .map(|data| {
            let bytes = data.to_vec();
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            Picture {
                id: hasher.finish(),
                bytes,
                mime: text(keys.mime),
            }
        });
}

/// Answers still to come for one look at MediaRemote.
struct Pending {
    now: Now,
    left: u8,
    done: Option<Box<dyn FnOnce(Now)>>,
}

/// One answer is in: it fills its part, and the last one hands the whole over.
fn answered(pending: &RefCell<Pending>, fill: impl FnOnce(&mut Now)) {
    let mut p = pending.borrow_mut();
    fill(&mut p.now);
    p.left = p.left.saturating_sub(1);
    if p.left == 0 {
        let (now, done) = (std::mem::take(&mut p.now), p.done.take());
        drop(p);
        if let Some(done) = done {
            done(now);
        }
    }
}

/// Asks MediaRemote for everything a line holds; `done` runs on the main queue once all four answers are in.
fn fetch(mr: &'static MediaRemote, done: impl FnOnce(Now) + 'static) {
    let pending = Rc::new(RefCell::new(Pending {
        now: Now {
            at_ms: wall_ms(),
            ..Now::default()
        },
        left: 4,
        done: Some(Box::new(done)),
    }));
    let p = Rc::clone(&pending);
    let info = RcBlock::new(move |dict: InfoReply| {
        autoreleasepool(|_| {
            // SAFETY: MediaRemote passes a valid dictionary, or NULL when nothing plays, for the length of the call.
            let dict = unsafe { dict.as_ref() };
            answered(&p, |now| {
                if let Some(dict) = dict {
                    read_info(&mr.keys, dict, now);
                }
            });
        });
    });
    let p = Rc::clone(&pending);
    let playing = RcBlock::new(move |playing: u8| answered(&p, |now| now.playing = playing != 0));
    let p = Rc::clone(&pending);
    let pid = RcBlock::new(move |pid: c_int| answered(&p, |now| now.pid = pid));
    let p = pending;
    let client = RcBlock::new(move |client: *mut AnyObject| {
        autoreleasepool(|_| {
            answered(&p, |now| {
                if client.is_null() {
                    return;
                }
                // SAFETY: `client` is the live client object for the length of the call; the getters follow the
                // Get rule (nothing to release) and return a CFString or NULL.
                let (bundle, parent) = unsafe {
                    (
                        (mr.client_bundle)(client).as_ref(),
                        (mr.client_parent)(client).as_ref(),
                    )
                };
                now.bundle = bundle.map_or_else(String::new, |s| s.to_string());
                now.parent = parent.map_or_else(String::new, |s| s.to_string());
            });
        });
    });
    let queue = DispatchQueue::main();
    // SAFETY: the functions have these signatures; MediaRemote copies each block and calls it once on `queue`.
    unsafe {
        (mr.info)(queue, &info);
        (mr.playing)(queue, &playing);
        (mr.pid)(queue, &pid);
        (mr.client)(queue, &client);
    }
}

/// Whether `new` says something `old` did not: any field, or a position other than the one `old` predicts for
/// now (a seek). A position that moves along as predicted is not news.
fn is_news(old: Option<&Now>, new: &Now) -> bool {
    let Some(old) = old else {
        return true;
    };
    let same = old.playing == new.playing
        && old.title == new.title
        && old.artist == new.artist
        && old.album == new.album
        && old.duration == new.duration
        && old.rate == new.rate
        && old.bundle == new.bundle
        && old.parent == new.parent
        && old.pid == new.pid
        && old.picture_id() == new.picture_id();
    if !same {
        return true;
    }
    match (old.elapsed, new.elapsed) {
        (Some(was), Some(is)) => {
            let expected = was + old.speed() * (new.at_ms - old.at_ms) / 1000.0;
            (is - expected).abs() > DRIFT_SECONDS
        }
        (was, is) => was.is_some() != is.is_some(),
    }
}

fn push_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The line for `now`. `picture` is `Some` when the artwork changed since the previous line: the new picture, or
/// `None` when it went away.
fn json_line(now: &Now, picture: Option<Option<&Picture>>) -> String {
    let mut out = String::with_capacity(
        320 + now
            .artwork
            .as_ref()
            .map_or(0, |p| p.bytes.len().div_ceil(3) * 4),
    );
    out.push_str("{\"playing\":");
    out.push_str(if now.playing { "true" } else { "false" });
    for (key, value) in [
        ("title", &now.title),
        ("artist", &now.artist),
        ("album", &now.album),
    ] {
        let _ = write!(out, ",\"{key}\":");
        push_string(&mut out, value);
    }
    for (key, value) in [
        ("duration", now.duration),
        ("elapsed", now.elapsed),
        ("rate", now.rate),
    ] {
        match value {
            Some(v) if v.is_finite() => {
                let _ = write!(out, ",\"{key}\":{v}");
            }
            _ => {
                let _ = write!(out, ",\"{key}\":null");
            }
        }
    }
    let _ = write!(out, ",\"at_ms\":{:.0}", now.at_ms.max(0.0));
    for (key, value) in [("bundle", &now.bundle), ("parent", &now.parent)] {
        let _ = write!(out, ",\"{key}\":");
        push_string(&mut out, value);
    }
    let _ = write!(out, ",\"pid\":{}", now.pid);
    if let Some(picture) = picture {
        out.push_str(",\"artwork\":\"");
        out.push_str(&picture.map_or_else(String::new, |p| base64(&p.bytes)));
        out.push_str("\",\"mime\":");
        push_string(&mut out, picture.map_or("", |p| p.mime.as_str()));
    }
    out.push_str("}\n");
    out
}

/// Ends the process at once, without running anyone's exit handlers: perl is in the middle of a call into this
/// library, and MediaRemote has threads of its own.
fn exit_now(code: c_int) -> ! {
    // SAFETY: _exit has no preconditions and does not return.
    unsafe { libc::_exit(code) }
}

fn print(line: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(line.as_bytes())?;
    out.flush()
}

/// Prints `now` when it is news against the previous line; a closed stdout ends the process.
fn report(last: &RefCell<Option<Now>>, now: Now) {
    let mut last = last.borrow_mut();
    if !is_news(last.as_ref(), &now) {
        return;
    }
    let before = last.as_ref().and_then(Now::picture_id);
    let picture = (before != now.picture_id()).then_some(now.artwork.as_ref());
    if print(&json_line(&now, picture)).is_err() {
        exit_now(0);
    }
    *last = Some(now);
}

/// Streams Now Playing until stdout is closed or the parent process is gone; never returns. Called by perl as an
/// XSUB, whose two arguments (the interpreter and the sub) are not used.
#[unsafe(no_mangle)]
pub extern "C" fn figghost_np_stream(_perl: *mut c_void, _cv: *mut c_void) {
    // A write to a closed pipe must fail with EPIPE, which ends the stream below, instead of killing the process.
    // SAFETY: changing the disposition of SIGPIPE has no preconditions.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    let Some(mr) = MediaRemote::load() else {
        exit_now(1)
    };
    let last: Rc<RefCell<Option<Now>>> = Rc::default();
    let refresh = Rc::new(move || {
        let last = Rc::clone(&last);
        fetch(mr, move |now| report(&last, now));
    });
    // SAFETY: the function has this signature; notifications are delivered on the main queue, serviced below.
    unsafe { (mr.register)(DispatchQueue::main()) };
    let center = NSNotificationCenter::defaultCenter();
    let main = NSOperationQueue::mainQueue();
    let on_change = {
        let refresh = Rc::clone(&refresh);
        RcBlock::new(move |_: NonNull<NSNotification>| refresh())
    };
    let _observers: Vec<_> = mr
        .changes
        .iter()
        .map(|&name| {
            // SAFETY: the block is 'static and runs on the main operation queue, so on this thread.
            unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&main),
                    &on_change,
                )
            }
        })
        .collect();
    refresh();
    let orphan = RcBlock::new(|_: NonNull<NSTimer>| {
        // SAFETY: getppid has no preconditions. A process whose parent died belongs to launchd, pid 1.
        if unsafe { libc::getppid() } == 1 {
            exit_now(0);
        }
    });
    // SAFETY: the block is 'static; the timer goes on this thread's run loop, which runs below.
    let timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_repeats_block(PARENT_CHECK_SECONDS, true, &orphan)
    };
    timer.setTolerance(0.5);
    run_until(|| false, None);
    exit_now(0);
}

/// Prints one line, artwork included, and returns. For checks: `perl ... figghost_np_get`.
#[unsafe(no_mangle)]
pub extern "C" fn figghost_np_get(_perl: *mut c_void, _cv: *mut c_void) {
    let Some(mr) = MediaRemote::load() else {
        return;
    };
    let printed = Rc::new(Cell::new(false));
    let flag = Rc::clone(&printed);
    fetch(mr, move |now| {
        let _ = print(&json_line(&now, Some(now.artwork.as_ref())));
        flag.set(true);
    });
    run_until(move || printed.get(), Some(Instant::now() + GET_TIMEOUT));
}

/// Moves what plays to the position in `FIGGHOST_NP_SEEK` (seconds from its start), then prints one line as
/// `figghost_np_get` does. The line comes from MediaRemote after the move was sent to it, so once it is printed the
/// move has left this process; the player may report the new position a moment later.
#[unsafe(no_mangle)]
pub extern "C" fn figghost_np_seek(perl: *mut c_void, cv: *mut c_void) {
    let Some(seconds) = std::env::var("FIGGHOST_NP_SEEK")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s >= 0.0)
    else {
        return;
    };
    if MediaRemote::load().is_none() {
        return;
    }
    // SAFETY: the framework is already loaded by `load` and stays loaded; looking a symbol up has no other effect.
    let found = unsafe {
        let lib = libc::dlopen(FRAMEWORK.as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
        if lib.is_null() {
            return;
        }
        libc::dlsym(lib, c"MRMediaRemoteSetElapsedTime".as_ptr())
    };
    if found.is_null() {
        return;
    }
    // SAFETY: MRMediaRemoteSetElapsedTime takes the position in seconds and returns nothing, as every MediaRemote
    // client declares it.
    let set_elapsed =
        unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(f64)>(found) };
    // SAFETY: see above; the argument is a finite position.
    unsafe { set_elapsed(seconds) };
    figghost_np_get(perl, cv);
}

/// Runs this thread's run loop, which serves the main queue, one event at a time, each in its own autorelease pool,
/// until `done` or the deadline. Waits without spinning: a pass returns only after an event.
fn run_until(done: impl Fn() -> bool, deadline: Option<Instant>) {
    let run_loop = NSRunLoop::currentRunLoop();
    // SAFETY: an extern constant of Foundation.
    let mode = unsafe { NSDefaultRunLoopMode };
    while !done() {
        let limit: Retained<NSDate> = match deadline {
            Some(at) => {
                let left = at.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return;
                }
                NSDate::dateWithTimeIntervalSinceNow(left.as_secs_f64())
            }
            None => NSDate::distantFuture(),
        };
        // False when the run loop has nothing to wait for, which would make this loop spin.
        if !autoreleasepool(|_| run_loop.runMode_beforeDate(mode, &limit)) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing(elapsed: f64, at_ms: f64) -> Now {
        Now {
            playing: true,
            title: "Song".into(),
            artist: "Band".into(),
            duration: Some(300.0),
            elapsed: Some(elapsed),
            rate: Some(1.0),
            at_ms,
            bundle: "com.apple.Music".into(),
            pid: 42,
            ..Now::default()
        }
    }

    #[test]
    fn the_first_look_is_always_news() {
        assert!(is_news(None, &Now::default()));
    }

    #[test]
    fn a_position_moving_as_predicted_is_not_news_but_a_seek_is() {
        let old = playing(10.0, 1_000_000.0);
        assert!(!is_news(Some(&old), &playing(15.2, 1_005_000.0)));
        assert!(!is_news(Some(&old), &playing(13.6, 1_005_000.0)));
        assert!(is_news(Some(&old), &playing(60.0, 1_005_000.0)));
        assert!(is_news(Some(&old), &playing(2.0, 1_005_000.0)));
        // Paused, the position must stay where it was.
        let paused = Now {
            playing: false,
            rate: Some(0.0),
            ..playing(10.0, 1_000_000.0)
        };
        let later = Now {
            playing: false,
            rate: Some(0.0),
            ..playing(10.0, 1_060_000.0)
        };
        assert!(!is_news(Some(&paused), &later));
        let moved = Now {
            playing: false,
            rate: Some(0.0),
            ..playing(40.0, 1_060_000.0)
        };
        assert!(is_news(Some(&paused), &moved));
        // Twice the speed predicts twice the distance.
        let fast = Now {
            rate: Some(2.0),
            ..playing(10.0, 1_000_000.0)
        };
        let ahead = Now {
            rate: Some(2.0),
            ..playing(20.0, 1_005_000.0)
        };
        assert!(!is_news(Some(&fast), &ahead));
    }

    #[test]
    fn any_other_change_is_news() {
        let old = playing(10.0, 1_000_000.0);
        let changes: [fn(&mut Now); 8] = [
            |n| n.playing = false,
            |n| n.title = "Other".into(),
            |n| n.duration = None,
            |n| n.elapsed = None,
            |n| n.rate = Some(1.5),
            |n| n.parent = "com.apple.Safari".into(),
            |n| n.pid = 7,
            |n| {
                n.artwork = Some(Picture {
                    id: 1,
                    bytes: vec![1],
                    mime: String::new(),
                })
            },
        ];
        for change in changes {
            let mut new = playing(10.0, 1_000_000.0);
            change(&mut new);
            assert!(is_news(Some(&old), &new));
        }
    }

    #[test]
    fn lines_escape_text_and_write_null_for_unknown_numbers() {
        let now = Now {
            title: "A \"quoted\" \\ title\nNhạc Việt".into(),
            elapsed: Some(1.5),
            rate: Some(f64::NAN),
            at_ms: 1_792_049_126_808.4,
            ..Now::default()
        };
        assert_eq!(
            json_line(&now, None),
            "{\"playing\":false,\"title\":\"A \\\"quoted\\\" \\\\ title\\u000aNhạc Việt\",\"artist\":\"\",\
             \"album\":\"\",\"duration\":null,\"elapsed\":1.5,\"rate\":null,\"at_ms\":1792049126808,\
             \"bundle\":\"\",\"parent\":\"\",\"pid\":0}\n"
        );
    }

    #[test]
    fn the_artwork_is_written_only_when_it_changed() {
        let picture = Picture {
            id: 9,
            bytes: b"foobar".to_vec(),
            mime: "image/jpeg".into(),
        };
        let now = Now::default();
        assert!(!json_line(&now, None).contains("artwork"));
        assert!(
            json_line(&now, Some(Some(&picture)))
                .ends_with(",\"artwork\":\"Zm9vYmFy\",\"mime\":\"image/jpeg\"}\n")
        );
        assert!(json_line(&now, Some(None)).ends_with(",\"artwork\":\"\",\"mime\":\"\"}\n"));
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected, "{input:?}");
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn the_last_answer_hands_the_whole_over_once() {
        let got = Rc::new(Cell::new(0));
        let seen = Rc::clone(&got);
        let pending = RefCell::new(Pending {
            now: Now::default(),
            left: 2,
            done: Some(Box::new(move |now: Now| {
                assert_eq!((now.pid, now.playing), (5, true));
                seen.set(seen.get() + 1);
            })),
        });
        answered(&pending, |n| n.pid = 5);
        assert_eq!(got.get(), 0);
        answered(&pending, |n| n.playing = true);
        answered(&pending, |n| n.pid = 6);
        assert_eq!(got.get(), 1);
    }
}
