//! What the Updates tab knows without running anything: reading what `brew`, `npm`, `mas`, `softwareupdate` and the
//! agents print, telling which version is newer, and the commands that update what is chosen. The tests feed it
//! answers in the shape the tools give.

use serde_json::Value;

const MAX_TEXT: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    Formula,
    Cask,
    Npm,
    AppStore,
    Apple,
    Agent,
}

/// One piece of software that has a newer version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub source: Source,
    /// What the command is given: a formula, an npm package, an App Store number, an agent's program.
    pub id: String,
    pub name: String,
    /// Empty when the version now is not known.
    pub current: String,
    pub latest: String,
}

impl Update {
    /// What a choice is remembered by, the same across looks.
    pub fn key(&self) -> String {
        format!("{:?}:{}", self.source, self.id)
    }
}

/// A coding agent that installs and updates itself, outside Homebrew and npm.
pub struct Agent {
    pub name: &'static str,
    pub program: &'static str,
    /// Where npm publishes its versions, which is how the newest one is known.
    pub package: &'static str,
    pub update: &'static [&'static str],
}

pub const AGENTS: [Agent; 5] = [
    Agent {
        name: "Claude Code",
        program: "claude",
        package: "@anthropic-ai/claude-code",
        update: &["update"],
    },
    Agent {
        name: "OpenCode",
        program: "opencode",
        package: "opencode-ai",
        update: &["upgrade"],
    },
    Agent {
        name: "Pi",
        program: "pi",
        package: "@earendil-works/pi-coding-agent",
        update: &["update", "self"],
    },
    Agent {
        name: "Droid",
        program: "droid",
        package: "droid",
        update: &["update"],
    },
    Agent {
        name: "Amp",
        program: "amp",
        package: "@sourcegraph/amp",
        update: &["update"],
    },
];

/// A path an agent's program resolves to that Homebrew or npm looks after, so that it is listed there and not twice.
pub fn managed_elsewhere(resolved: &str) -> bool {
    ["/Cellar/", "/Caskroom/", "/node_modules/"]
        .iter()
        .any(|part| resolved.contains(part))
}

fn tidy(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// The numbers of a version, for comparing: `1.10.0-beta.2` is `[1, 10, 0, 2]`.
fn numbers(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().unwrap_or(u64::MAX))
        .collect()
}

/// Whether `latest` is a newer version than `current`. A version with no numbers is never newer.
pub fn newer(latest: &str, current: &str) -> bool {
    let (a, b) = (numbers(latest), numbers(current));
    !a.is_empty() && a > b
}

/// The first version in what a program says of itself: `2.1.3 (Claude Code)` or `opencode v2.0.32`.
pub fn version_in(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
        .map(|word| word.trim_start_matches(['v', 'V']))
        .find(|word| {
            word.starts_with(|c: char| c.is_ascii_digit())
                && word.contains('.')
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c))
        })
        .map(tidy)
}

/// What `brew outdated --json=v2` printed. Pinned formulae are left alone.
pub fn parse_brew(body: &str) -> Result<Vec<Update>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (key, source) in [("formulae", Source::Formula), ("casks", Source::Cask)] {
        for item in v[key].as_array().into_iter().flatten() {
            let (Some(name), Some(latest)) =
                (item["name"].as_str(), item["current_version"].as_str())
            else {
                continue;
            };
            if item["pinned"].as_bool() == Some(true) {
                continue;
            }
            let current = item["installed_versions"]
                .as_array()
                .and_then(|a| a.last())
                .and_then(Value::as_str)
                .unwrap_or_default();
            out.push(Update {
                source,
                id: tidy(name),
                name: tidy(name),
                current: tidy(current),
                latest: tidy(latest),
            });
        }
    }
    Ok(out)
}

/// What `npm outdated -g --json` printed: each package with the version installed and the newest one.
pub fn parse_npm(body: &str) -> Result<Vec<Update>, String> {
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let map = v.as_object().ok_or("not an object")?;
    Ok(map
        .iter()
        .filter_map(|(name, item)| {
            let (current, latest) = (item["current"].as_str()?, item["latest"].as_str()?);
            (current != latest).then(|| Update {
                source: Source::Npm,
                id: tidy(name),
                name: tidy(name),
                current: tidy(current),
                latest: tidy(latest),
            })
        })
        .collect())
}

/// What `mas outdated` printed: `497799835  Xcode  (15.0 -> 15.1)` on each line.
pub fn parse_mas(body: &str) -> Vec<Update> {
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (id, rest) = line.split_once(char::is_whitespace)?;
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let open = rest.rfind('(')?;
            let (from, to) = rest[open + 1..].trim_end_matches(')').split_once("->")?;
            Some(Update {
                source: Source::AppStore,
                id: id.to_owned(),
                name: tidy(&rest[..open]),
                current: tidy(from),
                latest: tidy(to),
            })
        })
        .collect()
}

/// What `softwareupdate --list` printed: `Title: macOS Sequoia 15.5, Version: 15.5, Size: ...` on each update.
/// `macos` is the version of macOS running, the one now of a macOS update.
pub fn parse_softwareupdate(body: &str, macos: &str) -> Vec<Update> {
    body.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Title:")?;
            let mut title = "";
            let mut version = "";
            for part in rest.split(", ") {
                if let Some(v) = part.trim().strip_prefix("Version:") {
                    version = v.trim();
                } else if title.is_empty() {
                    title = part.trim();
                }
            }
            if title.is_empty() || version.is_empty() {
                return None;
            }
            Some(Update {
                source: Source::Apple,
                id: tidy(title),
                name: tidy(title),
                current: if title.starts_with("macOS") {
                    tidy(macos)
                } else {
                    String::new()
                },
                latest: tidy(version),
            })
        })
        .collect()
}

/// The newest version in what `registry.npmjs.org/<package>/latest` answered.
pub fn npm_latest(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v["version"].as_str().map(tidy).filter(|s| !s.is_empty())
}

/// What updating a set of chosen updates means: the commands to run, each a program and its arguments, and whether
/// System Settings must be opened for the Apple ones, which need an administrator.
pub fn plan(chosen: &[Update]) -> (Vec<Vec<String>>, bool) {
    let ids = |source: Source| -> Vec<String> {
        chosen
            .iter()
            .filter(|u| u.source == source)
            .map(|u| u.id.clone())
            .collect()
    };
    let mut commands = Vec::new();
    for (source, head) in [
        (Source::Formula, &["brew", "upgrade", "--formula"][..]),
        (Source::Cask, &["brew", "upgrade", "--cask"][..]),
        (Source::AppStore, &["mas", "upgrade"][..]),
    ] {
        let list = ids(source);
        if !list.is_empty() {
            commands.push(head.iter().map(|s| (*s).to_owned()).chain(list).collect());
        }
    }
    let npm = ids(Source::Npm);
    if !npm.is_empty() {
        let mut command: Vec<String> = ["npm", "install", "-g"].map(String::from).to_vec();
        command.extend(npm.iter().map(|p| format!("{p}@latest")));
        commands.push(command);
    }
    for u in chosen.iter().filter(|u| u.source == Source::Agent) {
        if let Some(agent) = AGENTS.iter().find(|a| a.program == u.id) {
            let mut command = vec![agent.program.to_owned()];
            command.extend(agent.update.iter().map(|s| (*s).to_owned()));
            commands.push(command);
        }
    }
    (commands, chosen.iter().any(|u| u.source == Source::Apple))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_version_is_told_by_its_numbers() {
        assert!(newer("1.10.0", "1.9.9"));
        assert!(newer("2.1.310", "2.1.309"));
        assert!(newer("0.248.0", "0.245.0"));
        assert!(!newer("1.19.4", "2.0.32"), "an older one is never offered");
        assert!(!newer("1.2.0", "1.2.0"));
        assert!(!newer("", "1.0"));
        assert!(newer("1.0.1", ""));
    }

    #[test]
    fn the_version_a_program_says_is_found_in_its_words() {
        assert_eq!(
            version_in("2.1.309 (Claude Code)").as_deref(),
            Some("2.1.309")
        );
        assert_eq!(version_in("opencode v2.0.32").as_deref(), Some("2.0.32"));
        assert_eq!(version_in("codex-cli 0.46.0\n").as_deref(), Some("0.46.0"));
        assert_eq!(version_in("no version here"), None);
    }

    #[test]
    fn brew_formulae_and_casks_are_read_and_pinned_ones_left_out() {
        let body = r#"{"formulae":[
            {"name":"node","installed_versions":["22.1.0"],"current_version":"22.2.0","pinned":false},
            {"name":"git","installed_versions":["2.40"],"current_version":"2.41","pinned":true}],
          "casks":[{"name":"codex","installed_versions":["0.163.0"],"current_version":"0.170.0"}]}"#;
        let list = parse_brew(body).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (
                list[0].source,
                list[0].current.as_str(),
                list[0].latest.as_str()
            ),
            (Source::Formula, "22.1.0", "22.2.0")
        );
        assert_eq!(list[1].source, Source::Cask);
        assert!(parse_brew("not json").is_err());
    }

    #[test]
    fn npm_packages_that_are_behind_are_read() {
        let body = r#"{"@openai/codex":{"current":"0.1.0","wanted":"0.2.0","latest":"0.2.0"},
                       "npm":{"current":"11.0.0","latest":"11.0.0"}}"#;
        let list = parse_npm(body).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "@openai/codex");
        assert!(
            parse_npm("").unwrap().is_empty(),
            "npm prints nothing when all is current"
        );
    }

    #[test]
    fn app_store_and_apple_updates_are_read_from_their_lines() {
        let mas = parse_mas("497799835  Xcode  (15.0 -> 15.1)\nnonsense\n");
        assert_eq!(mas.len(), 1);
        assert_eq!(
            (
                mas[0].id.as_str(),
                mas[0].name.as_str(),
                mas[0].current.as_str(),
                mas[0].latest.as_str()
            ),
            ("497799835", "Xcode", "15.0", "15.1")
        );
        let apple = parse_softwareupdate(
            "Software Update found the following new or updated software:\n* Label: macOS Sequoia 15.5-24F74\n\tTitle: macOS Sequoia 15.5, Version: 15.5, Size: 5G, Recommended: YES, Action: restart,\n* Label: Safari\n\tTitle: Safari, Version: 18.5, Size: 100K, Recommended: YES,\n",
            "15.4",
        );
        assert_eq!(apple.len(), 2);
        assert_eq!(
            (apple[0].current.as_str(), apple[0].latest.as_str()),
            ("15.4", "15.5")
        );
        assert_eq!(apple[1].current, "", "only macOS has a known version now");
    }

    #[test]
    fn the_plan_groups_by_tool_and_sends_apple_to_system_settings() {
        let u = |source, id: &str| Update {
            source,
            id: id.into(),
            name: id.into(),
            current: "1".into(),
            latest: "2".into(),
        };
        let (commands, apple) = plan(&[
            u(Source::Formula, "node"),
            u(Source::Formula, "git"),
            u(Source::Npm, "@openai/codex"),
            u(Source::Agent, "claude"),
            u(Source::Apple, "Safari"),
        ]);
        assert!(apple);
        assert_eq!(commands[0], ["brew", "upgrade", "--formula", "node", "git"]);
        assert_eq!(
            commands[1],
            ["npm", "install", "-g", "@openai/codex@latest"]
        );
        assert_eq!(commands[2], ["claude", "update"]);
        assert_eq!(commands.len(), 3);
        assert_eq!(plan(&[]), (Vec::<Vec<String>>::new(), false));
    }

    #[test]
    fn programs_that_homebrew_or_npm_look_after_are_not_listed_as_agents() {
        assert!(managed_elsewhere(
            "/opt/homebrew/Caskroom/codex/0.163.0/bin/codex"
        ));
        assert!(managed_elsewhere(
            "/opt/homebrew/lib/node_modules/@google/gemini-cli/dist/index.js"
        ));
        assert!(!managed_elsewhere(
            "/Users/me/.local/share/claude/versions/2.1.309"
        ));
    }
}
