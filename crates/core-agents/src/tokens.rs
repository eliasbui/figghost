//! Tokens and money the coding agents spent, added up from the logs they keep themselves: Claude Code writes
//! every reply with its token counts into a transcript, Codex writes running totals into its session logs.
//! There are thousands of files and gigabytes of them, so a ledger reads each file once, from where its
//! previous read stopped, and keeps only what falls inside the window it is asked for. `refresh` reads the
//! disk, and the first one takes about a second, so call it off the thread that draws.

use crate::usage::{newest_rollouts, parse_rfc3339_ms};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

/// A price row covers prompts of up to this many tokens; only one model costs more for longer prompts.
const ANY_PROMPT: u64 = u64::MAX;

/// USD per million tokens, from the official Anthropic pricing page fetched on 2026-10-15; change it here when
/// the page changes. Columns: model id, the longest prompt the row covers, then base input, 5-minute cache
/// write, 1-hour cache write, cache read and output.
#[rustfmt::skip]
const PRICES: &[(&str, u64, [f64; 5])] = &[
    ("claude-fable-5-1", ANY_PROMPT, [10.0, 12.50, 20.0, 0.25, 50.0]),
    ("claude-mythos-5-1", ANY_PROMPT, [10.0, 12.50, 20.0, 0.25, 50.0]),
    ("claude-fable-5", ANY_PROMPT, [10.0, 12.50, 20.0, 1.0, 50.0]),
    ("claude-mythos-5", ANY_PROMPT, [10.0, 12.50, 20.0, 1.0, 50.0]),
    ("claude-opus-5-5", ANY_PROMPT, [4.0, 5.0, 8.0, 0.20, 20.0]),
    ("claude-opus-5", ANY_PROMPT, [5.0, 6.25, 10.0, 0.50, 25.0]),
    ("claude-opus-4-8", ANY_PROMPT, [5.0, 6.25, 10.0, 0.50, 25.0]),
    ("claude-opus-4-7", ANY_PROMPT, [5.0, 6.25, 10.0, 0.50, 25.0]),
    ("claude-opus-4-6", ANY_PROMPT, [5.0, 6.25, 10.0, 0.50, 25.0]),
    ("claude-opus-4-5", ANY_PROMPT, [5.0, 6.25, 10.0, 0.50, 25.0]),
    ("claude-opus-4-1", ANY_PROMPT, [15.0, 18.75, 30.0, 1.50, 75.0]),
    ("claude-opus-4", ANY_PROMPT, [15.0, 18.75, 30.0, 1.50, 75.0]),
    ("claude-opus-4-0", ANY_PROMPT, [15.0, 18.75, 30.0, 1.50, 75.0]),
    ("claude-sonnet-5-5", ANY_PROMPT, [2.0, 2.50, 4.0, 0.10, 10.0]),
    ("claude-sonnet-5", ANY_PROMPT, [2.0, 2.50, 4.0, 0.20, 10.0]),
    ("claude-sonnet-4-6", ANY_PROMPT, [3.0, 3.75, 6.0, 0.30, 15.0]),
    ("claude-sonnet-4-5", ANY_PROMPT, [3.0, 3.75, 6.0, 0.30, 15.0]),
    ("claude-sonnet-4", ANY_PROMPT, [3.0, 3.75, 6.0, 0.30, 15.0]),
    ("claude-sonnet-4-0", ANY_PROMPT, [3.0, 3.75, 6.0, 0.30, 15.0]),
    ("claude-haiku-5-5", 100_000, [0.10, 0.125, 0.20, 0.01, 0.50]),
    ("claude-haiku-5-5", ANY_PROMPT, [0.50, 0.625, 1.0, 0.05, 2.50]),
    ("claude-haiku-4-5", ANY_PROMPT, [1.0, 1.25, 2.0, 0.10, 5.0]),
    ("claude-3-5-haiku", ANY_PROMPT, [0.80, 1.0, 1.60, 0.08, 4.0]),
];

/// Big enough that a log is read in few calls, small next to the lines that have to be held whole anyway.
const CHUNK_BYTES: usize = 256 * 1024;

/// What was spent over some stretch of time. Tokens are split the way they are billed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Totals {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    /// Only messages with a price add to it.
    pub cost_usd: f64,
    /// Replies of Claude Code, token counts that added something for Codex. A reply whose usage is all zeros
    /// still counts: some Claude Code versions write them that way.
    pub messages: u64,
    /// Messages of a model without a price: their tokens are in the counters but not in `cost_usd`.
    pub unpriced: u64,
}

impl Totals {
    pub fn tokens(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_write)
            .saturating_add(self.cache_read)
    }

    // The counters come from files anyone can edit, so a silly one must not overflow into a panic.
    fn add(&mut self, spend: &Spend) {
        self.input = self.input.saturating_add(spend.input);
        self.output = self.output.saturating_add(spend.output);
        self.cache_write = self.cache_write.saturating_add(spend.cache_write);
        self.cache_read = self.cache_read.saturating_add(spend.cache_read);
        self.messages += 1;
        match spend.cost_usd {
            Some(cost) => self.cost_usd += cost,
            None => self.unpriced += 1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub today: Totals,
    /// The last seven days, today included.
    pub week: Totals,
    /// Today per model: the one that cost most first, then the one with most tokens.
    pub models_today: Vec<(String, Totals)>,
    /// When the newest message was written, in milliseconds since 1970; 0 when there is none.
    pub latest_ms: i64,
}

/// The model id the way the price table spells it: lowercase, without a `[1m]` or `@date` suffix and without
/// the snapshot date some ids end with, so `claude-sonnet-4-20250514[1m]` becomes `claude-sonnet-4`.
fn normalize_model(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    let name = lower.split(['[', '@']).next().unwrap_or_default();
    match name.rsplit_once('-') {
        Some((base, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => {
            base.to_owned()
        }
        _ => name.to_owned(),
    }
}

/// USD per million tokens for a normalized model id and the size of the prompt, or `None` without a price.
/// Ids are compared whole, so `claude-sonnet-5` never picks up the price of `claude-sonnet-5-5`.
fn price(model: &str, prompt_tokens: u64) -> Option<[f64; 5]> {
    PRICES
        .iter()
        .find(|(id, longest_prompt, _)| *id == model && prompt_tokens <= *longest_prompt)
        .map(|(_, _, usd)| *usd)
}

/// A reply and a request id tell one message from another; Claude Code writes the same pair once for each
/// content block of the message. The pair is kept as its hash: a week holds tens of thousands of messages, and two
/// ids of some forty characters each would cost more memory than everything else a record holds.
type Key = u64;

/// What one message used, after merging all its copies.
#[derive(Debug, Clone)]
struct Record {
    /// When the first copy was written.
    at_ms: i64,
    /// Normalized, see `normalize_model`; shared by every record of the same model.
    model: Arc<str>,
    fast: bool,
    input: u64,
    output: u64,
    /// All cache writes, of which `write_1h` lasted an hour and the rest five minutes.
    write: u64,
    write_1h: u64,
    read: u64,
}

impl Record {
    /// Later copies of a message only ever grow its counters, so the largest of each is the final one. Early
    /// copies often lack the speed, so a copy that says fast makes the whole message fast.
    fn merge(&mut self, copy: &Record) {
        self.fast |= copy.fast;
        self.input = self.input.max(copy.input);
        self.output = self.output.max(copy.output);
        self.write = self.write.max(copy.write);
        self.write_1h = self.write_1h.max(copy.write_1h);
        self.read = self.read.max(copy.read);
    }

    /// What the message cost in USD, or `None` for a model without a price. Fast mode costs twice as much.
    fn cost_usd(&self) -> Option<f64> {
        let write_5m = self.write.saturating_sub(self.write_1h);
        let prompt = self
            .input
            .saturating_add(self.write)
            .saturating_add(self.read);
        let [input, five_minutes, one_hour, read, output] = price(&self.model, prompt)?;
        let usd = self.input as f64 * input
            + write_5m as f64 * five_minutes
            + self.write_1h as f64 * one_hour
            + self.read as f64 * read
            + self.output as f64 * output;
        Some(usd / 1e6 * if self.fast { 2.0 } else { 1.0 })
    }

    fn spend(&self) -> Spend<'_> {
        Spend {
            at_ms: self.at_ms,
            model: Some(&self.model),
            input: self.input,
            output: self.output,
            cache_write: self.write,
            cache_read: self.read,
            cost_usd: self.cost_usd(),
        }
    }
}

/// The few fields of a transcript line that matter. Serde skips the rest, which can be megabytes of tool output.
#[derive(Deserialize)]
struct TranscriptLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    uuid: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<Reply>,
}

#[derive(Deserialize)]
struct Reply {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation: Option<CacheCreation>,
    speed: Option<String>,
}

#[derive(Deserialize, Default)]
struct CacheCreation {
    ephemeral_5m_input_tokens: Option<u64>,
    ephemeral_1h_input_tokens: Option<u64>,
}

/// The message a transcript line reports, with the key that tells its copies apart; `None` for any other line.
fn claude_record_from_line(
    text: &str,
    intern: &mut impl FnMut(String) -> Arc<str>,
) -> Option<(Key, Record)> {
    // Most lines are tool output, some of it megabytes. A quote inside a JSON string is escaped, so a quoted
    // word is only found where it is a real key or value, and lines without both words are not worth parsing.
    if !text.contains("\"assistant\"") || !text.contains("\"usage\"") {
        return None;
    }
    let line: TranscriptLine = serde_json::from_str(text).ok()?;
    if line.kind.as_deref() != Some("assistant") {
        return None;
    }
    let reply = line.message?;
    let usage = reply.usage?;
    let at_ms = parse_rfc3339_ms(&line.timestamp?)?;
    // A reply without its own id can still be told apart by the id of the line that holds it.
    let mut hasher = DefaultHasher::new();
    (reply.id.or(line.uuid)?, line.request_id.unwrap_or_default()).hash(&mut hasher);
    let key = hasher.finish();
    let split = usage.cache_creation.unwrap_or_default();
    let five_minutes = split.ephemeral_5m_input_tokens.unwrap_or(0);
    let one_hour = split.ephemeral_1h_input_tokens.unwrap_or(0);
    let record = Record {
        at_ms,
        model: intern(normalize_model(reply.model.as_deref().unwrap_or("unknown"))),
        fast: usage.speed.as_deref() == Some("fast"),
        input: usage.input_tokens.unwrap_or(0),
        output: usage.output_tokens.unwrap_or(0),
        // The split only says how long the writes last, without it all of them count as 5-minute writes. The
        // total is the larger of the two figures so that a line holding only one of them still counts it all.
        write: usage
            .cache_creation_input_tokens
            .unwrap_or(0)
            .max(five_minutes.saturating_add(one_hour)),
        write_1h: one_hour,
        read: usage.cache_read_input_tokens.unwrap_or(0),
    };
    Some((key, record))
}

fn read_claude_line(
    records: &mut HashMap<Key, Record>,
    models: &mut Vec<Arc<str>>,
    since_ms: i64,
    line: &[u8],
) {
    let mut intern = |name: String| -> Arc<str> {
        if let Some(known) = models.iter().find(|m| ***m == *name) {
            return Arc::clone(known);
        }
        let name: Arc<str> = Arc::from(name);
        models.push(Arc::clone(&name));
        name
    };
    let Some((key, copy)) = std::str::from_utf8(line)
        .ok()
        .and_then(|text| claude_record_from_line(text, &mut intern))
    else {
        return;
    };
    match records.get_mut(&key) {
        Some(known) => known.merge(&copy),
        // Messages from before the window are not worth keeping.
        None if copy.at_ms >= since_ms => {
            records.insert(key, copy);
        }
        None => {}
    }
}

/// Every transcript below `dir`: the sessions of a project, the agents a session started and the agents of the
/// workflows those started, which sit six folders down. Links are not followed: some only point at another
/// transcript of the same tree.
fn claude_transcripts(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            claude_transcripts(&path, found);
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            found.push(path);
        }
    }
}

/// What Claude Code spent, from the transcripts under `~/.claude/projects`.
#[derive(Debug)]
pub struct ClaudeLedger {
    projects_dir: PathBuf,
    cursors: Cursors<()>,
    /// One record per message, whichever file it came from: a resumed session starts a new file that repeats
    /// the history of the old one. Records stay until they age out of the window, since what a message cost
    /// is spent whether or not its file is still there.
    records: HashMap<Key, Record>,
    /// The model names seen, each kept once.
    models: Vec<Arc<str>>,
}

impl ClaudeLedger {
    pub fn new(projects_dir: PathBuf) -> Self {
        Self {
            projects_dir,
            cursors: Cursors::default(),
            records: HashMap::new(),
            models: Vec::new(),
        }
    }

    /// Reads what the transcripts gained since the last call and forgets what is older than `since_ms`.
    /// Moving `since_ms` back does not bring back what an earlier call dropped.
    pub fn refresh(&mut self, since_ms: i64) {
        let mut paths = Vec::new();
        claude_transcripts(&self.projects_dir, &mut paths);
        let (records, models) = (&mut self.records, &mut self.models);
        self.cursors.read_new(paths, since_ms, |(), line| {
            read_claude_line(records, models, since_ms, line)
        });
        self.records.retain(|_, record| record.at_ms >= since_ms);
    }

    /// The caller passes the start of the local day and of the 7-day window, this crate knows no time zone.
    pub fn summary(&self, day_start_ms: i64, week_start_ms: i64) -> Summary {
        summarize(
            self.records.values().map(Record::spend),
            day_start_ms,
            week_start_ms,
        )
    }
}

/// What a Codex log has told so far.
#[derive(Debug, Default)]
struct CodexFile {
    /// The running totals at the last count read: input, cached input, cache write and output tokens.
    totals: Option<[u64; 4]>,
    events: Vec<Spend<'static>>,
}

/// The time and the running totals of a Codex `token_count` line; `None` for any other line and for counts
/// that carry no usable totals. A count that is damaged is dropped whole: reading it as zeros would make the
/// next good count look like a fresh start and add everything again.
fn codex_totals_from_line(text: &str) -> Option<(i64, [u64; 4])> {
    if !text.contains("\"total_token_usage\"") {
        return None;
    }
    let line: Value = serde_json::from_str(text).ok()?;
    if line.get("type")?.as_str()? != "event_msg"
        || line.pointer("/payload/type")?.as_str()? != "token_count"
    {
        return None;
    }
    let totals = line
        .pointer("/payload/info/total_token_usage")?
        .as_object()?;
    // Input and output must be there; the two cache counters are left out by logs that never used a cache.
    let optional = |name: &str| totals.get(name).map_or(Some(0), Value::as_u64);
    let at_ms = parse_rfc3339_ms(line.get("timestamp")?.as_str()?)?;
    let input = totals.get("input_tokens")?.as_u64()?;
    let output = totals.get("output_tokens")?.as_u64()?;
    let totals = [
        input,
        optional("cached_input_tokens")?,
        optional("cache_write_input_tokens")?,
        output,
    ];
    Some((at_ms, totals))
}

fn read_codex_line(file: &mut CodexFile, since_ms: i64, line: &[u8]) {
    let Some((at_ms, totals)) = std::str::from_utf8(line)
        .ok()
        .and_then(codex_totals_from_line)
    else {
        return;
    };
    // Even a count older than the window sets the base the next one is measured from.
    let before = file.totals.replace(totals);
    // Totals only grow within a session, so one that went back started over and all of it is new.
    let spent: [u64; 4] = match before {
        Some(before) if before.iter().zip(&totals).all(|(was, now)| was <= now) => {
            std::array::from_fn(|i| totals[i] - before[i])
        }
        _ => totals,
    };
    // The same count is written again whenever the limits change, and then nothing was spent.
    if at_ms >= since_ms && spent != [0; 4] {
        let [input, cached, cache_write, output] = spent;
        file.events.push(Spend {
            at_ms,
            model: None,
            // Cached tokens are part of the input count, so the rest is what was billed as plain input.
            input: input.saturating_sub(cached),
            output,
            cache_write,
            cache_read: cached,
            cost_usd: None,
        });
    }
}

/// What Codex used, from the session logs under `~/.codex/sessions`. It has no price here, so every count is
/// unpriced and `models_today` stays empty, the counts do not say which model they belong to.
#[derive(Debug)]
pub struct CodexLedger {
    sessions_dir: PathBuf,
    /// Counts are differences between running totals, so they belong to the log they were read from: a log
    /// that gets rewritten or removed takes its counts along.
    cursors: Cursors<CodexFile>,
}

impl CodexLedger {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self {
            sessions_dir,
            cursors: Cursors::default(),
        }
    }

    /// Reads what the logs gained since the last call and forgets what is older than `since_ms`.
    /// Moving `since_ms` back does not bring back what an earlier call dropped.
    pub fn refresh(&mut self, since_ms: i64) {
        let mut paths = Vec::new();
        newest_rollouts(&self.sessions_dir, 0, usize::MAX, &mut paths);
        self.cursors.read_new(paths, since_ms, |file, line| {
            read_codex_line(file, since_ms, line)
        });
        for cursor in self.cursors.files.values_mut() {
            cursor.state.events.retain(|event| event.at_ms >= since_ms);
        }
    }

    /// The caller passes the start of the local day and of the 7-day window, this crate knows no time zone.
    pub fn summary(&self, day_start_ms: i64, week_start_ms: i64) -> Summary {
        let events = self
            .cursors
            .files
            .values()
            .flat_map(|cursor| cursor.state.events.iter().copied());
        summarize(events, day_start_ms, week_start_ms)
    }
}

/// One message or event as the summary sees it.
#[derive(Debug, Clone, Copy)]
struct Spend<'a> {
    at_ms: i64,
    /// Codex does not say.
    model: Option<&'a str>,
    input: u64,
    output: u64,
    cache_write: u64,
    cache_read: u64,
    /// `None` without a price.
    cost_usd: Option<f64>,
}

fn summarize<'a>(
    spends: impl Iterator<Item = Spend<'a>>,
    day_start_ms: i64,
    week_start_ms: i64,
) -> Summary {
    let mut summary = Summary::default();
    let mut models: HashMap<&str, Totals> = HashMap::new();
    for spend in spends {
        summary.latest_ms = summary.latest_ms.max(spend.at_ms);
        if spend.at_ms >= week_start_ms {
            summary.week.add(&spend);
        }
        if spend.at_ms >= day_start_ms {
            summary.today.add(&spend);
            if let Some(model) = spend.model {
                models.entry(model).or_default().add(&spend);
            }
        }
    }
    summary.models_today = models
        .into_iter()
        .map(|(model, totals)| (model.to_owned(), totals))
        .collect();
    // The name last, so that equal models always come out in the same order.
    summary.models_today.sort_by(|(a_name, a), (b_name, b)| {
        b.cost_usd
            .total_cmp(&a.cost_usd)
            .then(b.tokens().cmp(&a.tokens()))
            .then_with(|| a_name.cmp(b_name))
    });
    summary
}

/// Where the last read of one log stopped, and what the reader keeps about that log between reads.
#[derive(Debug, Default)]
struct Cursor<S> {
    /// Just past the last complete line that was handed out.
    offset: u64,
    state: S,
}

#[derive(Debug, Default)]
struct Cursors<S> {
    files: HashMap<PathBuf, Cursor<S>>,
}

impl<S: Default> Cursors<S> {
    /// Hands `each_line` every complete line that `paths` gained since the previous call, with the state of
    /// the log it is from. A log nobody wrote to since `since_ms` is skipped and, like any log that is not in
    /// `paths`, forgotten, so a long-running app holds nothing for files that left the window. A log shorter
    /// than before was rewritten and starts over with a fresh state. A last line without its newline is still
    /// being written and is left for the next call.
    fn read_new(
        &mut self,
        paths: Vec<PathBuf>,
        since_ms: i64,
        mut each_line: impl FnMut(&mut S, &[u8]),
    ) {
        let oldest = UNIX_EPOCH + Duration::from_millis(since_ms.max(0).unsigned_abs());
        let mut kept = HashMap::new();
        for path in paths {
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            if meta.modified().is_ok_and(|written| written < oldest) {
                continue;
            }
            let mut cursor = self.files.remove(&path).unwrap_or_default();
            if meta.len() < cursor.offset {
                cursor = Cursor::default();
            }
            if meta.len() > cursor.offset {
                // A log that cannot be read now is tried again on the next call, from the same place.
                let _ = read_lines(&path, &mut cursor, &mut each_line);
            }
            kept.insert(path, cursor);
        }
        self.files = kept;
    }
}

/// Hands out the complete lines of `path` from the cursor on and moves the cursor past each one.
fn read_lines<S>(
    path: &Path,
    cursor: &mut Cursor<S>,
    each_line: &mut impl FnMut(&mut S, &[u8]),
) -> io::Result<()> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(cursor.offset))?;
    let mut reader = BufReader::with_capacity(CHUNK_BYTES, file);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = reader.read_until(b'\n', &mut line)?;
        if line.last() != Some(&b'\n') {
            // The end of the file, or a line that is still being written and is read again once it is whole.
            return Ok(());
        }
        cursor.offset += read as u64;
        each_line(&mut cursor.state, &line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::Write,
        time::{Instant, SystemTime},
    };

    const DAY_MS: i64 = 86_400_000;
    const NOW: &str = "2026-10-15T07:00:00.000Z";

    /// A line shaped like the ones Claude Code writes (checked against a real transcript).
    const REAL_LINE: &str = r#"{"type":"assistant","timestamp":"2026-10-15T07:22:39.704Z","requestId":"req_1","uuid":"u1","message":{"id":"msg_1","model":"claude-opus-5-5","usage":{"input_tokens":2,"cache_creation_input_tokens":26355,"cache_read_input_tokens":26141,"output_tokens":516,"cache_creation":{"ephemeral_1h_input_tokens":26355,"ephemeral_5m_input_tokens":0},"speed":"standard"}}}"#;

    fn ms(text: &str) -> i64 {
        parse_rfc3339_ms(text).unwrap()
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{actual} is not {expected}"
        );
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("core-agents-tokens-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Makes the file look as if it was last written at that moment.
    fn touch(path: &Path, at: &str) {
        let at = UNIX_EPOCH + Duration::from_millis(ms(at).unsigned_abs());
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(at)
            .unwrap();
    }

    fn write_lines(path: &Path, lines: &[String]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, lines.join("\n") + "\n").unwrap();
    }

    fn append(path: &Path, text: &str) {
        let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    /// Usage the way Claude Code writes it, with the cache writes split by how long they last.
    fn usage(input: u64, write_5m: u64, write_1h: u64, read: u64, output: u64) -> Value {
        json!({
            "input_tokens": input,
            "cache_creation_input_tokens": write_5m + write_1h,
            "cache_read_input_tokens": read,
            "output_tokens": output,
            "cache_creation": {
                "ephemeral_5m_input_tokens": write_5m,
                "ephemeral_1h_input_tokens": write_1h
            },
            "speed": "standard"
        })
    }

    fn reply(at: &str, message: &str, model: &str, usage: Value) -> String {
        json!({
            "type": "assistant", "timestamp": at,
            "uuid": format!("line-{message}"), "requestId": format!("req-{message}"),
            "message": { "id": message, "model": model, "usage": usage }
        })
        .to_string()
    }

    /// A ledger that has read these lines from one fresh transcript.
    fn claude_ledger(name: &str, lines: &[String]) -> ClaudeLedger {
        let dir = scratch(name);
        write_lines(&dir.join("project/session.jsonl"), lines);
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        let _ = fs::remove_dir_all(&dir);
        ledger
    }

    fn record(
        model: &str,
        input: u64,
        write_5m: u64,
        write_1h: u64,
        read: u64,
        output: u64,
    ) -> Record {
        Record {
            at_ms: 0,
            model: Arc::from(normalize_model(model)),
            fast: false,
            input,
            output,
            write: write_5m + write_1h,
            write_1h,
            read,
        }
    }

    #[test]
    fn model_ids_lose_their_case_suffixes_and_dates() {
        for (raw, expected) in [
            ("claude-sonnet-5-5", "claude-sonnet-5-5"),
            ("claude-sonnet-5-5[1m]", "claude-sonnet-5-5"),
            ("Claude-Opus-5-5[1M]", "claude-opus-5-5"),
            ("claude-sonnet-4-20250514", "claude-sonnet-4"),
            ("claude-opus-4-1-20250805", "claude-opus-4-1"),
            ("claude-sonnet-4@20250514", "claude-sonnet-4"),
            ("claude-sonnet-4-20250514[1m]", "claude-sonnet-4"),
            ("claude-3-5-haiku-20241022", "claude-3-5-haiku"),
            ("claude-sonnet-5-1234567", "claude-sonnet-5-1234567"),
            ("<synthetic>", "<synthetic>"),
        ] {
            assert_eq!(normalize_model(raw), expected, "{raw}");
        }
    }

    #[test]
    fn ids_that_start_alike_are_priced_apart() {
        let read_price = |model: &str| price(&normalize_model(model), 0).unwrap()[3];
        assert_eq!(read_price("claude-sonnet-5"), 0.20);
        assert_eq!(read_price("claude-sonnet-5-5"), 0.10);
        assert_eq!(read_price("claude-opus-5"), 0.50);
        assert_eq!(read_price("claude-opus-5-5"), 0.20);
        assert_eq!(read_price("claude-fable-5"), 1.0);
        assert_eq!(read_price("claude-fable-5-1[1m]"), 0.25);
        assert_eq!(read_price("claude-opus-4-1-20250805"), 1.50);
        assert_eq!(read_price("claude-sonnet-4-20250514"), 0.30);
        assert_eq!(price("claude-opus-4-0", 0), price("claude-opus-4", 0));
        assert_eq!(price("claude-sonnet-4-0", 0), price("claude-sonnet-4", 0));
        assert_eq!(price("claude-mythos-5-1", 0), price("claude-fable-5-1", 0));
    }

    #[test]
    fn models_without_a_price_have_none() {
        for model in [
            "<synthetic>",
            "gpt-5",
            "claude-sonnet-9",
            "claude-sonnet",
            "claude-3-opus",
            "",
        ] {
            assert_eq!(price(&normalize_model(model), 0), None, "{model}");
        }
        assert_eq!(record("<synthetic>", 5, 0, 0, 0, 5).cost_usd(), None);
    }

    #[test]
    fn every_row_of_the_price_table_charges_cache_writes_at_the_usual_multiples_of_input() {
        // Five-minute writes cost 1.25 times the input price and one-hour writes twice that. This catches a
        // typo in the table; if the pricing page ever breaks the pattern, change it along with the table.
        for (id, _, [input, write_5m, write_1h, ..]) in PRICES {
            let usual =
                (write_5m - input * 1.25).abs() < 1e-9 && (write_1h - input * 2.0).abs() < 1e-9;
            assert!(usual, "{id}");
        }
    }

    #[test]
    fn haiku_5_5_costs_more_once_the_prompt_passes_100000_tokens() {
        assert_eq!(
            price("claude-haiku-5-5", 100_000),
            Some([0.10, 0.125, 0.20, 0.01, 0.50])
        );
        assert_eq!(
            price("claude-haiku-5-5", 100_001),
            Some([0.50, 0.625, 1.0, 0.05, 2.50])
        );
        let cost = |input, write_5m, write_1h, read, output| {
            record("claude-haiku-5-5", input, write_5m, write_1h, read, output)
                .cost_usd()
                .unwrap()
        };
        // A prompt of 51000 tokens: 1000 * 0.10 + 50000 * 0.01 + 1000 * 0.50 per million.
        close(cost(1_000, 0, 0, 50_000, 1_000), 0.0011);
        // A prompt of 150000 tokens: 50000 * 0.50 + 100000 * 0.05 + 1000 * 2.50 per million.
        close(cost(50_000, 0, 0, 100_000, 1_000), 0.0325);
        // Cache writes are part of the prompt too: 60000 * 0.625 + 50000 * 1.0 per million.
        close(cost(0, 60_000, 50_000, 0, 0), 0.0875);
        // What the model answers is not.
        close(cost(0, 0, 0, 0, 500_000), 0.25);
    }

    #[test]
    fn hour_writes_cost_more_than_five_minute_writes_and_fast_mode_doubles_the_price() {
        let cost = |record: &Record| record.cost_usd().unwrap();
        // Opus 5.5 charges 4 for input, 5 for a 5-minute write, 8 for a 1-hour write and 20 for output.
        close(cost(&record("claude-opus-5-5", 0, 1_000_000, 0, 0, 0)), 5.0);
        close(cost(&record("claude-opus-5-5", 0, 0, 1_000_000, 0, 0)), 8.0);
        let mut both = record("claude-opus-5-5", 1_000_000, 0, 0, 0, 1_000_000);
        close(cost(&both), 24.0);
        both.fast = true;
        close(cost(&both), 48.0);
        // The first copies of a message often have no speed yet, and which copy comes first must not matter.
        let copy = |usage: Value| reply(NOW, "m1", "claude-opus-5-5", usage);
        let early = copy(json!({"input_tokens": 1_000_000}));
        let fast = copy(json!({"input_tokens": 1_000_000, "speed": "fast"}));
        for lines in [[early.clone(), fast.clone()], [fast, early]] {
            close(
                claude_ledger("fast", &lines).summary(0, 0).week.cost_usd,
                8.0,
            );
        }
    }

    #[test]
    fn one_known_line_costs_what_the_price_list_says() {
        let summary = claude_ledger("known-line", &[REAL_LINE.to_owned()]).summary(0, 0);
        let week = &summary.week;
        assert_eq!(
            (week.input, week.output, week.cache_write, week.cache_read),
            (2, 516, 26_355, 26_141)
        );
        assert_eq!((week.messages, week.unpriced), (1, 0));
        // 2 input at 4, 26355 one-hour writes at 8, 26141 reads at 0.20 and 516 output at 20, per million.
        close(
            week.cost_usd,
            (2.0 * 4.0 + 26_355.0 * 8.0 + 26_141.0 * 0.20 + 516.0 * 20.0) / 1e6,
        );
        close(week.cost_usd, 0.226_396_2);
        assert_eq!(summary.latest_ms, ms("2026-10-15T07:22:39.704Z"));
        assert_eq!(summary.models_today.len(), 1);
        assert_eq!(summary.models_today[0].0, "claude-opus-5-5");
    }

    #[test]
    fn writes_without_a_split_are_five_minute_writes_and_a_larger_split_is_trusted() {
        let total_only = json!({"cache_creation_input_tokens": 1_000_000, "cache_creation": null, "speed": null});
        let summary = claude_ledger(
            "no-split",
            &[reply(NOW, "m1", "claude-opus-5-5", total_only)],
        )
        .summary(0, 0);
        assert_eq!(summary.week.cache_write, 1_000_000);
        close(summary.week.cost_usd, 5.0);
        // Real transcripts have a few lines whose total is 0 next to a split that says otherwise.
        let odd = json!({
            "cache_creation_input_tokens": 0,
            "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 2_000_000}
        });
        let summary =
            claude_ledger("odd-split", &[reply(NOW, "m1", "claude-opus-5-5", odd)]).summary(0, 0);
        assert_eq!(summary.week.cache_write, 2_000_000);
        close(summary.week.cost_usd, 16.0);
    }

    #[test]
    fn copies_that_disagree_about_the_split_do_not_count_the_writes_twice() {
        let split = reply(NOW, "m1", "claude-opus-5-5", usage(0, 0, 1_000_000, 0, 0));
        let total_only = reply(
            NOW,
            "m1",
            "claude-opus-5-5",
            json!({"cache_creation_input_tokens": 1_000_000}),
        );
        for lines in [[split.clone(), total_only.clone()], [total_only, split]] {
            let summary = claude_ledger("mixed-copies", &lines).summary(0, 0);
            assert_eq!(summary.week.cache_write, 1_000_000);
            close(summary.week.cost_usd, 8.0);
        }
    }

    #[test]
    fn copies_of_one_message_count_once_with_their_largest_counters_and_first_time() {
        let dir = scratch("copies");
        let path = dir.join("project/session.jsonl");
        let copy =
            |at: &str, output| reply(at, "m1", "claude-opus-5-5", usage(2, 0, 100, 50, output));
        // Claude Code writes the message again for each of its content blocks, and the output counter grows.
        write_lines(
            &path,
            &[
                copy("2026-10-14T23:59:59Z", 10),
                copy("2026-10-15T00:00:05Z", 40),
                copy("2026-10-15T00:00:06Z", 25),
            ],
        );
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        let midnight = ms("2026-10-15T00:00:00Z");
        let summary = ledger.summary(midnight, 0);
        assert_eq!(
            (
                summary.week.messages,
                summary.week.input,
                summary.week.output
            ),
            (1, 2, 40)
        );
        assert_eq!(
            summary.today.messages, 0,
            "it belongs to when its first copy was written"
        );
        // Another copy turns up in a later read of the same file.
        append(&path, &format!("{}\n", copy("2026-10-15T00:00:07Z", 120)));
        ledger.refresh(0);
        let summary = ledger.summary(midnight, 0);
        assert_eq!((summary.week.messages, summary.week.output), (1, 120));
        assert_eq!(ledger.records.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_message_repeated_in_a_resumed_session_counts_once() {
        let dir = scratch("resumed");
        let message =
            |id: &str, output| reply(NOW, id, "claude-opus-5-5", usage(1, 0, 0, 0, output));
        let old = [message("m1", 10), message("m2", 20)];
        write_lines(&dir.join("project/old.jsonl"), &old);
        // A resumed session starts a file of its own that repeats the history and goes on from it.
        let resumed = [old[0].clone(), old[1].clone(), message("m3", 30)];
        write_lines(&dir.join("project/old/subagents/agent-1.jsonl"), &resumed);
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        let week = ledger.summary(0, 0).week;
        assert_eq!((week.messages, week.output), (3, 60));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_reply_without_a_message_id_is_told_apart_by_the_id_of_its_line() {
        let bare = |uuid: Option<&str>| {
            let mut line = json!({
                "type": "assistant", "timestamp": NOW,
                "message": {"model": "claude-opus-5-5", "usage": usage(1, 0, 0, 0, 1)}
            });
            if let Some(uuid) = uuid {
                line["uuid"] = json!(uuid);
            }
            line.to_string()
        };
        let lines = [
            bare(Some("a")),
            bare(Some("a")),
            bare(Some("b")),
            bare(None),
        ];
        let summary = claude_ledger("bare-replies", &lines).summary(0, 0);
        assert_eq!(
            summary.week.messages, 2,
            "a and b count, the one with no id at all does not"
        );
    }

    #[test]
    fn a_last_line_without_its_newline_waits_until_it_is_complete() {
        let dir = scratch("partial");
        let path = dir.join("project/session.jsonl");
        let first = reply(NOW, "m1", "claude-opus-5-5", usage(1, 0, 0, 0, 10));
        let second = reply(NOW, "m2", "claude-opus-5-5", usage(1, 0, 0, 0, 20));
        let (head, tail) = second.split_at(second.len() / 2);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("{first}\n{head}")).unwrap();
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0).week.messages, 1);
        assert_eq!(ledger.cursors.files[&path].offset, first.len() as u64 + 1);
        append(&path, &format!("{tail}\n"));
        ledger.refresh(0);
        let week = ledger.summary(0, 0).week;
        assert_eq!((week.messages, week.output), (2, 30));
        assert_eq!(
            ledger.cursors.files[&path].offset,
            fs::metadata(&path).unwrap().len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_gets_shorter_is_read_again_from_its_start() {
        let dir = scratch("shrink");
        let path = dir.join("project/session.jsonl");
        let message = |id: &str| reply(NOW, id, "claude-opus-5-5", usage(1, 0, 0, 0, 10));
        write_lines(&path, &[message("m1"), message("m2")]);
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0).week.messages, 2);
        // Rewritten with less in it. What the earlier messages cost was spent, so they stay counted.
        write_lines(&path, &[message("m3")]);
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0).week.messages, 3);
        assert_eq!(
            ledger.cursors.files[&path].offset,
            fs::metadata(&path).unwrap().len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn files_not_written_since_the_window_began_are_skipped() {
        let dir = scratch("old-files");
        let old = dir.join("project/old.jsonl");
        let fresh = dir.join("project/fresh.jsonl");
        // Both hold a message inside the window, so only the age of the file can keep the old one out.
        let message = |id: &str| {
            reply(
                "2026-10-14T00:00:00Z",
                id,
                "claude-opus-5-5",
                usage(1, 0, 0, 0, 1),
            )
        };
        write_lines(&old, &[message("old")]);
        write_lines(&fresh, &[message("fresh")]);
        touch(&old, "2020-01-01T00:00:00Z");
        touch(&fresh, "2026-10-14T12:00:00Z");
        let since = ms("2026-10-01T00:00:00Z");
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(since);
        assert_eq!(ledger.summary(0, 0).week.messages, 1);
        assert_eq!(ledger.cursors.files.len(), 1);
        // Once something writes to it again it is read, from its start.
        touch(&old, "2026-10-14T13:00:00Z");
        ledger.refresh(since);
        assert_eq!(ledger.summary(0, 0).week.messages, 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vanished_file_is_forgotten_and_a_new_one_in_its_place_is_read_from_its_start() {
        let dir = scratch("vanished");
        let path = dir.join("project/session.jsonl");
        let message = |id: &str| reply(NOW, id, "claude-opus-5-5", usage(1, 0, 0, 0, 10));
        write_lines(&path, &[message("m1")]);
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        assert_eq!(ledger.cursors.files.len(), 1);
        fs::remove_file(&path).unwrap();
        ledger.refresh(0);
        assert!(ledger.cursors.files.is_empty());
        assert_eq!(
            ledger.summary(0, 0).week.messages,
            1,
            "what was spent stays until it ages out"
        );
        // Longer than the old file was, so a stale position would make it lose its first line.
        write_lines(&path, &[message("m2"), message("m3")]);
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0).week.messages, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_week_includes_today_and_messages_older_than_the_window_are_dropped() {
        let dir = scratch("windows");
        let path = dir.join("project/session.jsonl");
        let at = |time: &str, id: &str| reply(time, id, "claude-opus-5-5", usage(1, 0, 0, 0, 1));
        write_lines(
            &path,
            &[
                at("2026-10-08T23:59:59Z", "too-old"),
                at("2026-10-09T00:00:00Z", "week-start"),
                at("2026-10-14T23:59:59Z", "yesterday"),
                at("2026-10-15T00:00:00Z", "day-start"),
                at("2026-10-15T09:00:00Z", "later"),
            ],
        );
        touch(&path, "2026-10-15T12:00:00Z");
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        let day = ms("2026-10-15T00:00:00Z");
        let week = ms("2026-10-09T00:00:00Z");
        let summary = ledger.summary(day, week);
        assert_eq!((summary.today.messages, summary.week.messages), (2, 4));
        assert_eq!(summary.latest_ms, ms("2026-10-15T09:00:00Z"));
        // Starting the window later lets go of what fell out of it, which keeps the memory bounded.
        ledger.refresh(week);
        assert_eq!(ledger.records.len(), 4);
        assert_eq!(ledger.summary(0, 0).week.messages, 4);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_models_of_today_come_by_cost_then_tokens_and_unpriced_ones_keep_their_tokens() {
        let message = |time: &str, id: &str, model: &str, input| {
            reply(time, id, model, usage(input, 0, 0, 0, 0))
        };
        let today = "2026-10-15T08:00:00Z";
        let lines = [
            message(today, "a", "claude-sonnet-5-5[1m]", 1_000_000),
            message(today, "b", "claude-sonnet-5-5", 500_000),
            message(today, "c", "claude-opus-5-5", 1_000_000),
            message(today, "d", "gpt-x", 5_000_000),
            message(today, "e", "<synthetic>", 10),
            message("2026-10-14T08:00:00Z", "f", "claude-opus-5-5", 1_000_000),
        ];
        let summary = claude_ledger("models", &lines).summary(ms("2026-10-15T00:00:00Z"), 0);
        let names: Vec<_> = summary
            .models_today
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "claude-opus-5-5",
                "claude-sonnet-5-5",
                "gpt-x",
                "<synthetic>"
            ]
        );
        let sonnet = &summary.models_today[1].1;
        assert_eq!((sonnet.input, sonnet.messages), (1_500_000, 2));
        close(sonnet.cost_usd, 3.0);
        let unpriced = &summary.models_today[2].1;
        assert_eq!(
            (unpriced.tokens(), unpriced.unpriced, unpriced.cost_usd),
            (5_000_000, 1, 0.0)
        );
        assert_eq!((summary.today.messages, summary.today.unpriced), (5, 2));
        assert_eq!(summary.today.tokens(), 7_500_010);
        close(summary.today.cost_usd, 7.0);
        assert_eq!(summary.week.messages, 6);
        close(summary.week.cost_usd, 11.0);
    }

    #[test]
    fn hostile_lines_are_skipped_and_the_good_one_still_counts() {
        let mut bytes: Vec<u8> = Vec::new();
        for line in [
            b"garbage".as_slice(),
            b"",
            br#"{"type":"assistant""#,
            br#""assistant" "usage" in a line that is no json"#,
            br#"{"type":"assistant","message":"text","usage":{}}"#,
            br#"{"type":"assistant","timestamp":"2026-10-15T07:00:00Z","uuid":"a","message":{"model":"m","usage":{"input_tokens":"many","output_tokens":-3}}}"#,
            br#"{"type":"assistant","timestamp":"2026-10-15T07:00:00Z","uuid":"b","message":{"model":"m","usage":null}}"#,
            br#"{"type":"assistant","timestamp":"yesterday","uuid":"c","message":{"model":"m","usage":{"input_tokens":1}}}"#,
            br#"{"type":"assistant","timestamp":"99999999999999-01-01T00:00:00Z","uuid":"e","message":{"model":"m","usage":{"input_tokens":1}}}"#,
            br#"{"type":"assistant","timestamp":"2026-10-15T07:00:00+99999999999999:00","uuid":"f","message":{"model":"m","usage":{"input_tokens":1}}}"#,
            br#"{"type":"user","timestamp":"2026-10-15T07:00:00Z","uuid":"d","message":{"role":"assistant","usage":{"input_tokens":9}}}"#,
            b"\xff\xfe \"assistant\" \"usage\"",
        ] {
            bytes.extend_from_slice(line);
            bytes.push(b'\n');
        }
        // Megabytes that have one of the two words but not the other.
        bytes.extend_from_slice(b"\"assistant\" ");
        bytes.resize(bytes.len() + 3_000_000, b'x');
        bytes.push(b'\n');
        bytes.extend_from_slice(
            reply(NOW, "good", "claude-opus-5-5", usage(1, 0, 0, 0, 1)).as_bytes(),
        );
        bytes.push(b'\n');
        let dir = scratch("hostile");
        fs::create_dir_all(dir.join("project")).unwrap();
        fs::write(dir.join("project/session.jsonl"), bytes).unwrap();
        let mut ledger = ClaudeLedger::new(dir.clone());
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0).week.messages, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn absurd_counters_saturate_instead_of_overflowing() {
        let huge = json!({
            "input_tokens": u64::MAX, "output_tokens": u64::MAX,
            "cache_read_input_tokens": u64::MAX, "cache_creation_input_tokens": u64::MAX,
            "cache_creation": {
                "ephemeral_5m_input_tokens": u64::MAX, "ephemeral_1h_input_tokens": u64::MAX
            }
        });
        let lines = [
            reply(NOW, "m1", "claude-opus-5-5", huge.clone()),
            reply(NOW, "m2", "claude-opus-5-5", huge),
        ];
        let week = claude_ledger("absurd", &lines).summary(0, 0).week;
        assert_eq!(week.tokens(), u64::MAX);
        assert!(week.cost_usd.is_finite());
    }

    #[test]
    fn thousands_of_lines_and_a_line_longer_than_the_read_buffer_all_count() {
        let message = |id: String| reply(NOW, &id, "claude-opus-5-5", usage(1, 0, 0, 0, 1));
        let mut lines: Vec<String> = (0..4_000).map(|n| message(format!("m{n}"))).collect();
        // A message whose line is padded to more than a read buffer, in the middle of the file.
        let mut long: Value = serde_json::from_str(&lines[0]).unwrap();
        long["message"]["id"] = json!("long");
        long["padding"] = json!("x".repeat(CHUNK_BYTES * 2 + 5));
        lines.insert(2_000, long.to_string());
        let summary = claude_ledger("many-lines", &lines).summary(0, 0);
        assert_eq!(summary.week.messages, 4_001);
    }

    #[test]
    fn transcripts_are_found_in_sessions_agents_and_workflows_but_links_are_not_followed() {
        let dir = scratch("layout");
        for file in [
            "project/session.jsonl",
            "project/session/subagents/agent-1.jsonl",
            "project/session/subagents/workflows/wf_1/agent-2.jsonl",
            "project/session/tool-results/output.txt",
            "project/notes.md",
        ] {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        std::os::unix::fs::symlink(
            dir.join("project/session/subagents/agent-1.jsonl"),
            dir.join("project/session/subagents/agent-alias.jsonl"),
        )
        .unwrap();
        let mut found = Vec::new();
        claude_transcripts(&dir, &mut found);
        let mut names: Vec<_> = found
            .iter()
            .map(|path| {
                path.strip_prefix(&dir)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "project/session.jsonl",
                "project/session/subagents/agent-1.jsonl",
                "project/session/subagents/workflows/wf_1/agent-2.jsonl",
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn folders_that_do_not_exist_give_an_empty_summary() {
        let nowhere =
            std::env::temp_dir().join(format!("core-agents-tokens-nowhere-{}", std::process::id()));
        let mut claude = ClaudeLedger::new(nowhere.clone());
        let mut codex = CodexLedger::new(nowhere);
        claude.refresh(0);
        codex.refresh(0);
        assert_eq!(claude.summary(0, 0), Summary::default());
        assert_eq!(codex.summary(0, 0), Summary::default());
    }

    /// A line shaped like the ones Codex writes (checked against a real log).
    fn token_count(at: &str, input: u64, cached: u64, cache_write: u64, output: u64) -> String {
        json!({
            "timestamp": at, "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": input, "cached_input_tokens": cached,
                        "cache_write_input_tokens": cache_write, "output_tokens": output,
                        "reasoning_output_tokens": 0, "total_tokens": input + output
                    },
                    "last_token_usage": {}
                }
            }
        })
        .to_string()
    }

    fn codex_log(dir: &Path, day: &str, name: &str) -> PathBuf {
        dir.join(format!(
            "2026/10/{day}/rollout-2026-10-{day}T03-00-00-{name}.jsonl"
        ))
    }

    #[test]
    fn codex_totals_become_amounts_and_a_repeated_count_adds_nothing() {
        let dir = scratch("codex-deltas");
        let log = codex_log(&dir, "09", "a");
        write_lines(
            &log,
            &[
                r#"{"timestamp":"2026-10-09T03:00:00Z","type":"session_meta","payload":{}}"#.to_owned(),
                token_count("2026-10-09T03:01:00Z", 1_000, 600, 0, 50),
                // The same count again, as written whenever the limits change.
                token_count("2026-10-09T03:02:00Z", 1_000, 600, 0, 50),
                r#"{"timestamp":"2026-10-09T03:02:30Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{}}}"#.to_owned(),
                r#"{"timestamp":"2026-10-09T03:02:40Z","type":"event_msg","payload":{"type":"agent_message","message":"total_token_usage"}}"#.to_owned(),
                token_count("2026-10-09T03:03:00Z", 1_800, 1_100, 20, 90),
            ],
        );
        // Another session counts from zero on its own, not from where the first one stands.
        write_lines(
            &codex_log(&dir, "08", "b"),
            &[token_count("2026-10-08T03:01:00Z", 500, 0, 0, 5)],
        );
        let mut ledger = CodexLedger::new(dir.clone());
        ledger.refresh(0);
        let summary = ledger.summary(0, 0);
        // Counts one and two: input 1000 of which 600 cached, output 50. Then 800 more input of which 500
        // cached, 20 written to the cache and 40 output. Then the other session: 500 input and 5 output.
        let week = &summary.week;
        assert_eq!(
            (week.input, week.cache_read, week.cache_write, week.output),
            (1_200, 1_100, 20, 95)
        );
        assert_eq!((week.messages, week.unpriced, week.cost_usd), (3, 3, 0.0));
        assert!(summary.models_today.is_empty());
        assert_eq!(summary.latest_ms, ms("2026-10-09T03:03:00Z"));
        // Reading again changes nothing, a new count adds only what is new.
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0), summary);
        append(
            &log,
            &format!(
                "{}\n",
                token_count("2026-10-09T03:04:00Z", 2_000, 1_100, 20, 100)
            ),
        );
        ledger.refresh(0);
        let week = ledger.summary(0, 0).week;
        assert_eq!((week.input, week.output, week.messages), (1_400, 105, 4));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_codex_total_that_goes_back_starts_over() {
        let dir = scratch("codex-reset");
        write_lines(
            &codex_log(&dir, "09", "a"),
            &[
                token_count("2026-10-09T03:01:00Z", 100, 0, 0, 10),
                token_count("2026-10-09T03:02:00Z", 150, 0, 0, 30),
                token_count("2026-10-09T03:03:00Z", 20, 0, 0, 5),
                token_count("2026-10-09T03:04:00Z", 30, 0, 0, 9),
            ],
        );
        let mut ledger = CodexLedger::new(dir.clone());
        ledger.refresh(0);
        let week = ledger.summary(0, 0).week;
        // 100 + 50, then the restart at 20, then 10 more; output 10 + 20, then 5, then 4.
        assert_eq!((week.input, week.output, week.messages), (180, 39, 4));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_codex_counts_are_skipped_and_leave_the_base_alone() {
        let dir = scratch("codex-damaged");
        let damaged = |info: &str| {
            format!(
                r#"{{"timestamp":"2026-10-09T03:02:00Z","type":"event_msg","payload":{{"type":"token_count","info":{info}}}}}"#
            )
        };
        write_lines(
            &codex_log(&dir, "09", "a"),
            &[
                token_count("2026-10-09T03:01:00Z", 1_000, 0, 0, 100),
                damaged("null"),
                damaged(r#"{"total_token_usage":7}"#),
                damaged(r#"{"total_token_usage":{}}"#),
                damaged(r#"{"total_token_usage":{"input_tokens":"x","output_tokens":-1}}"#),
                damaged(
                    r#"{"total_token_usage":{"input_tokens":5,"output_tokens":5,"cached_input_tokens":null}}"#,
                ),
                token_count("never", 1_200, 0, 0, 120),
                "garbage".to_owned(),
                token_count("2026-10-09T03:03:00Z", 1_500, 0, 0, 160),
            ],
        );
        let mut ledger = CodexLedger::new(dir.clone());
        ledger.refresh(0);
        let week = ledger.summary(0, 0).week;
        // Had a damaged count set the base to zero, the last one would have added its whole 1500 again.
        assert_eq!((week.input, week.output, week.messages), (1_500, 160, 2));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn codex_counts_before_the_window_still_set_the_base_for_the_next() {
        let dir = scratch("codex-window");
        let log = codex_log(&dir, "09", "a");
        write_lines(
            &log,
            &[
                token_count("2026-10-01T00:00:00Z", 1_000, 0, 0, 100),
                token_count("2026-10-14T00:00:00Z", 1_500, 0, 0, 160),
            ],
        );
        touch(&log, "2026-10-14T12:00:00Z");
        let mut ledger = CodexLedger::new(dir.clone());
        ledger.refresh(ms("2026-10-10T00:00:00Z"));
        let week = ledger.summary(0, 0).week;
        assert_eq!((week.input, week.output, week.messages), (500, 60, 1));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rewritten_codex_log_replaces_its_counts_and_a_removed_one_takes_them_along() {
        let dir = scratch("codex-rewrite");
        let log = codex_log(&dir, "09", "a");
        write_lines(
            &log,
            &[
                token_count("2026-10-09T03:01:00Z", 1_000, 0, 0, 100),
                token_count("2026-10-09T03:02:00Z", 2_000, 0, 0, 200),
            ],
        );
        let mut ledger = CodexLedger::new(dir.clone());
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0).week.input, 2_000);
        write_lines(&log, &[token_count("2026-10-09T03:05:00Z", 300, 0, 0, 30)]);
        ledger.refresh(0);
        let week = ledger.summary(0, 0).week;
        assert_eq!((week.input, week.output, week.messages), (300, 30, 1));
        fs::remove_file(&log).unwrap();
        ledger.refresh(0);
        assert_eq!(ledger.summary(0, 0), Summary::default());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Reads the real transcripts and session logs of whoever runs it and prints counts, totals and model
    /// names, never what a message says:
    /// `cargo test -p core-agents --release -- --ignored --nocapture real_logs`
    #[test]
    #[ignore = "reads the real logs of the person running it"]
    fn real_logs_of_the_last_eight_days_are_read_in_reasonable_time() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let now_ms = i64::try_from(since_epoch.as_millis()).unwrap();
        let since = now_ms - 8 * DAY_MS;
        let mut claude = ClaudeLedger::new(home.join(".claude/projects"));
        let mut codex = CodexLedger::new(home.join(".codex/sessions"));
        for pass in ["first", "second"] {
            let started = Instant::now();
            claude.refresh(since);
            println!("{pass} Claude refresh: {:?}", started.elapsed());
            let started = Instant::now();
            codex.refresh(since);
            println!("{pass} Codex refresh: {:?}", started.elapsed());
        }
        println!(
            "Claude: {} files followed, {} messages held; Codex: {} files followed",
            claude.cursors.files.len(),
            claude.records.len(),
            codex.cursors.files.len()
        );
        // This crate knows no time zone, so here today is the last 24 hours and the week the last 7 days.
        let (day, week) = (now_ms - DAY_MS, now_ms - 7 * DAY_MS);
        let started = Instant::now();
        let claude_summary = claude.summary(day, week);
        println!("Claude summary: {:?}", started.elapsed());
        for (name, summary) in [
            ("Claude", claude_summary),
            ("Codex", codex.summary(day, week)),
        ] {
            println!(
                "{name}: newest message {} minutes ago",
                (now_ms - summary.latest_ms) / 60_000
            );
            for (span, totals) in [
                ("last 24 hours", &summary.today),
                ("last 7 days", &summary.week),
            ] {
                println!(
                    "{name} {span}: {} messages ({} unpriced), input {}, output {}, cache write {}, cache read {}, {:.2} USD",
                    totals.messages,
                    totals.unpriced,
                    totals.input,
                    totals.output,
                    totals.cache_write,
                    totals.cache_read,
                    totals.cost_usd
                );
            }
            for (model, totals) in &summary.models_today {
                println!(
                    "{name} model {model}: {} messages, {} tokens, {:.2} USD",
                    totals.messages,
                    totals.tokens(),
                    totals.cost_usd
                );
            }
        }
    }
}
