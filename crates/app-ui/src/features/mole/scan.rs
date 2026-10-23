//! What the page lists, found on disk, and how each kind of item is removed. Everything runs off the main thread.

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::SystemTime,
};

/// Rows kept for one list, the biggest first; the rest are left out, not paged.
const MAX_ITEMS: usize = 300;
/// A cache smaller than this is not worth a row.
const MIN_CACHE: u64 = 1 << 20;

/// Folders under the home folder whose contents can be deleted; `true` lists each entry inside as its own row.
const CLEAN: [(&str, &str, bool); 10] = [
    ("Library/Caches", "cache", true),
    ("Library/Logs", "log", true),
    (".cache", "cache", true),
    ("Library/Developer/Xcode/DerivedData", "dev", false),
    ("Library/Developer/Xcode/iOS DeviceSupport", "dev", false),
    ("Library/Developer/CoreSimulator/Caches", "dev", false),
    (".npm/_cacache", "dev", false),
    (".gradle/caches", "dev", false),
    (".cargo/registry/cache", "dev", false),
    (".Trash", "trash", false),
];

/// Where projects usually live; the same folder named twice (the disk ignores case) is walked once.
const PROJECTS: [&str; 13] = [
    "Projects",
    "projects",
    "Developer",
    "Code",
    "code",
    "dev",
    "src",
    "workspace",
    "GitHub",
    "Documents/GitHub",
    "Documents",
    "Desktop",
    "repos",
];
const PROJECT_DEPTH: usize = 5;

const INSTALLER_PLACES: [&str; 3] = ["Downloads", "Desktop", "Documents"];
const INSTALLER_TYPES: [&str; 5] = ["dmg", "pkg", "mpkg", "iso", "xip"];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Clean,
    Purge,
    Installers,
    Apps,
    Analyze,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Clean,
        Kind::Purge,
        Kind::Installers,
        Kind::Apps,
        Kind::Analyze,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Clean => "clean",
            Kind::Purge => "purge",
            Kind::Installers => "installers",
            Kind::Apps => "apps",
            Kind::Analyze => "analyze",
        }
    }

    pub fn parse(text: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.label() == text)
    }
}

/// One row of a list. `note` is the group of a cache, the project of a build folder or the identifier of an app;
/// `extra` are the leftovers removed with an app.
#[derive(Clone)]
pub struct Found {
    pub path: PathBuf,
    pub name: String,
    pub note: String,
    pub size: u64,
    pub dir: bool,
    pub age_days: Option<u64>,
    pub extra: Vec<PathBuf>,
}

impl Found {
    fn new(path: PathBuf, note: &str) -> Found {
        let size = size_of(&path);
        Found::sized(path, note, size)
    }

    fn sized(path: PathBuf, note: &str, size: u64) -> Found {
        let meta = fs::symlink_metadata(&path).ok();
        Found {
            name: file_name(&path),
            note: note.to_owned(),
            size,
            dir: meta.as_ref().is_some_and(fs::Metadata::is_dir),
            age_days: meta.and_then(|m| days_since(m.modified().ok()?)),
            path,
            extra: Vec::new(),
        }
    }

    pub fn json(&self) -> Value {
        json!({
            "path": self.path, "name": self.name, "note": self.note, "size": self.size, "dir": self.dir,
            "age": self.age_days, "extra": self.extra.len(),
        })
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy())
        .into_owned()
}

fn days_since(time: SystemTime) -> Option<u64> {
    Some(SystemTime::now().duration_since(time).ok()?.as_secs() / 86_400)
}

fn children(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect()
}

/// The room a file or folder takes on disk, without following links.
pub fn size_of(path: &Path) -> u64 {
    size_until(path, &|| false)
}

/// The same, but given up (with what was counted so far) as soon as `stop` says so.
fn size_until(path: &Path, stop: &dyn Fn() -> bool) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        if stop() {
            break;
        }
        let Ok(meta) = fs::symlink_metadata(&p) else {
            continue;
        };
        total += meta.blocks() * 512;
        if meta.is_dir() {
            stack.extend(children(&p));
        }
    }
    total
}

fn biggest(mut list: Vec<Found>) -> Vec<Found> {
    list.sort_by_key(|f| std::cmp::Reverse(f.size));
    list.truncate(MAX_ITEMS);
    list
}

/// The rows of one list. `own_id` is this app, which is never offered for uninstalling.
pub fn list(kind: Kind, home: &Path, own_id: Option<&str>) -> Vec<Found> {
    match kind {
        Kind::Clean => clean(home),
        Kind::Purge => purge(home),
        Kind::Installers => installers(home),
        Kind::Apps => apps(home, own_id),
        Kind::Analyze => analyze(home, &|| false),
    }
}

fn clean(home: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    for (place, group, each) in CLEAN {
        let dir = home.join(place);
        if !dir.is_dir() {
            continue;
        }
        if each {
            out.extend(
                children(&dir)
                    .into_iter()
                    // The system's own caches are in use by it and mostly cannot be deleted anyway.
                    .filter(|p| !file_name(p).starts_with("com.apple."))
                    .map(|p| Found::new(p, group))
                    .filter(|f| f.size >= MIN_CACHE),
            );
        } else {
            let found = Found::new(dir, group);
            if found.size > 0 {
                out.push(found);
            }
        }
    }
    biggest(out)
}

/// A folder of a project that its tools make again: what it is called and what next to it says so.
fn rebuildable(dir: &Path) -> bool {
    let Some(parent) = dir.parent() else {
        return false;
    };
    let has = |name: &str| parent.join(name).exists();
    let has_ext = |ext: &str| {
        children(parent)
            .iter()
            .any(|p| p.extension().is_some_and(|e| e == ext))
    };
    match file_name(dir).as_str() {
        "node_modules" | ".next" | ".nuxt" | ".turbo" | ".dart_tool" => true,
        "target" => has("Cargo.toml") || has("pom.xml"),
        "build" => {
            has("build.gradle")
                || has("build.gradle.kts")
                || has("CMakeLists.txt")
                || has("pubspec.yaml")
        }
        ".gradle" => has("build.gradle") || has("build.gradle.kts") || has("settings.gradle"),
        "Pods" => has("Podfile"),
        ".venv" | "venv" => dir.join("pyvenv.cfg").is_file(),
        "bin" | "obj" => has_ext("csproj") || has_ext("fsproj"),
        _ => false,
    }
}

fn purge(home: &Path) -> Vec<Found> {
    let roots: BTreeSet<PathBuf> = PROJECTS
        .iter()
        .filter_map(|p| home.join(p).canonicalize().ok())
        .collect();
    let mut found = BTreeMap::new();
    for root in roots {
        let mut stack = vec![(root, 0)];
        while let Some((dir, depth)) = stack.pop() {
            for p in children(&dir) {
                let Ok(meta) = fs::symlink_metadata(&p) else {
                    continue;
                };
                if !meta.is_dir() || found.contains_key(&p) {
                    continue;
                }
                if rebuildable(&p) {
                    let project = p.parent().map(file_name).unwrap_or_default();
                    let mut item = Found::new(p.clone(), &project);
                    // How long ago the project, not its build folder, was touched.
                    item.age_days = p
                        .parent()
                        .and_then(|d| days_since(fs::metadata(d).ok()?.modified().ok()?));
                    found.insert(p, item);
                } else {
                    let name = file_name(&p);
                    if depth + 1 < PROJECT_DEPTH
                        && !name.starts_with('.')
                        && !name.ends_with(".app")
                        && name != "Library"
                    {
                        stack.push((p, depth + 1));
                    }
                }
            }
        }
    }
    biggest(found.into_values().collect())
}

fn installers(home: &Path) -> Vec<Found> {
    let mut found = BTreeMap::new();
    for place in INSTALLER_PLACES {
        let mut stack = vec![(home.join(place), 0)];
        while let Some((dir, depth)) = stack.pop() {
            for p in children(&dir) {
                let kind = p
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if INSTALLER_TYPES.contains(&kind.as_str()) {
                    let item = Found::new(p.clone(), &kind);
                    found.insert(p, item);
                } else if depth < 2 && p.is_dir() && !file_name(&p).starts_with('.') {
                    stack.push((p, depth + 1));
                }
            }
        }
    }
    biggest(found.into_values().collect())
}

fn apps(home: &Path, own_id: Option<&str>) -> Vec<Found> {
    let mut bundles = Vec::new();
    for root in [PathBuf::from("/Applications"), home.join("Applications")] {
        for p in children(&root) {
            if p.extension().is_some_and(|e| e == "app") {
                bundles.push(p);
            } else if p.is_dir() {
                bundles.extend(
                    children(&p)
                        .into_iter()
                        .filter(|q| q.extension().is_some_and(|e| e == "app")),
                );
            }
        }
    }
    let library = home.join("Library");
    let mut out = Vec::new();
    for path in bundles {
        let Some((id, name)) = macos_sys::workspace::bundle_info(&path) else {
            continue;
        };
        if id.starts_with("com.apple.") || Some(id.as_str()) == own_id {
            continue;
        }
        let mut item = Found::new(path, &id);
        item.name = name.clone();
        item.extra = leftovers(&library, &id, &name);
        item.size += item.extra.iter().map(|p| size_of(p)).sum::<u64>();
        out.push(item);
    }
    biggest(out)
}

/// What an app leaves in the Library of the user, by its identifier and its name. The sandbox containers of other
/// apps are left out: reading them makes macOS ask for access to the data of other apps.
fn leftovers(library: &Path, id: &str, name: &str) -> Vec<PathBuf> {
    let mut out = BTreeSet::new();
    for dir in [
        "Application Support",
        "Caches",
        "Logs",
        "HTTPStorages",
        "WebKit",
        "Application Scripts",
    ] {
        for key in [id, name] {
            out.insert(library.join(dir).join(key));
        }
    }
    out.insert(library.join(format!("Preferences/{id}.plist")));
    out.insert(library.join(format!("Saved Application State/{id}.savedState")));
    out.insert(library.join(format!("Cookies/{id}.binarycookies")));
    for (dir, prefix) in [
        ("LaunchAgents", id.to_owned()),
        ("Preferences/ByHost", format!("{id}.")),
    ] {
        out.extend(
            children(&library.join(dir))
                .into_iter()
                .filter(|p| file_name(p).starts_with(&prefix)),
        );
    }
    out.into_iter()
        .filter(|p| fs::symlink_metadata(p).is_ok())
        .collect()
}

/// What is inside one folder, each entry with everything under it, the entries walked side by side. A walk that
/// `stop`s gives back what it has, which the caller throws away.
// shortcut: one entry is walked by one thread, so a single huge folder (Library) is the long pole; split it below
// its first level if that is ever too slow.
pub fn analyze(dir: &Path, stop: &(dyn Fn() -> bool + Sync)) -> Vec<Found> {
    let paths = children(dir);
    let next = AtomicUsize::new(0);
    let found = Mutex::new(Vec::with_capacity(paths.len()));
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(paths.len());
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                while let Some(p) = paths.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if stop() {
                        break;
                    }
                    let item = Found::sized(p.clone(), "", size_until(p, stop));
                    found.lock().unwrap_or_else(|e| e.into_inner()).push(item);
                }
            });
        }
    });
    biggest(found.into_inner().unwrap_or_else(|e| e.into_inner()))
}

/// Removes one row: caches are emptied and build folders deleted, since their tools make them again; installers,
/// apps with their leftovers and what the disk view lists go to the Trash, where they can be put back.
pub fn remove(kind: Kind, item: &Found) -> std::io::Result<()> {
    match kind {
        Kind::Clean if item.dir => {
            let mut result = Ok(());
            for p in children(&item.path) {
                if let Err(e) = delete(&p) {
                    result = Err(e);
                }
            }
            result
        }
        Kind::Clean | Kind::Purge => delete(&item.path),
        Kind::Installers | Kind::Analyze => macos_sys::workspace::trash(&item.path),
        Kind::Apps => {
            macos_sys::workspace::trash(&item.path)?;
            // The app is gone once it is in the Trash; a leftover that stays is not worth failing for.
            for p in &item.extra {
                let _ = macos_sys::workspace::trash(p);
            }
            Ok(())
        }
    }
}

fn delete(path: &Path) -> std::io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("figghost-mole-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn build_folders_are_found_only_beside_what_makes_them() {
        let home = scratch("purge");
        let app = home.join("Projects/app");
        fs::create_dir_all(app.join("node_modules/x")).unwrap();
        fs::write(app.join("node_modules/x/a.js"), vec![0; 5000]).unwrap();
        fs::create_dir_all(app.join("target")).unwrap();
        fs::create_dir_all(home.join("Projects/rust/target")).unwrap();
        fs::write(home.join("Projects/rust/Cargo.toml"), "").unwrap();
        let found: Vec<String> = purge(&home)
            .iter()
            .map(|f| {
                f.path
                    .strip_prefix(home.canonicalize().unwrap())
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();
        assert!(
            found.contains(&"Projects/app/node_modules".to_owned()),
            "{found:?}"
        );
        assert!(
            found.contains(&"Projects/rust/target".to_owned()),
            "{found:?}"
        );
        assert!(
            !found.iter().any(|f| f == "Projects/app/target"),
            "a target without Cargo.toml stays: {found:?}"
        );
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn cleaning_a_cache_empties_it_and_keeps_the_folder() {
        let home = scratch("clean");
        let cache = home.join("Library/Caches/org.example");
        fs::create_dir_all(cache.join("sub")).unwrap();
        fs::write(cache.join("sub/blob"), vec![1; 2 << 20]).unwrap();
        fs::create_dir_all(home.join("Library/Caches/com.apple.Safari")).unwrap();
        fs::write(
            home.join("Library/Caches/com.apple.Safari/blob"),
            vec![1; 2 << 20],
        )
        .unwrap();
        let list = clean(&home);
        assert_eq!(list.len(), 1, "the system's caches are not offered");
        assert!(list[0].size >= 2 << 20);
        remove(Kind::Clean, &list[0]).unwrap();
        assert!(cache.is_dir() && children(&cache).is_empty());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_disk_view_sizes_every_entry_and_a_stopped_walk_is_cut_short() {
        let dir = scratch("analyze");
        fs::create_dir_all(dir.join("big/deep")).unwrap();
        fs::write(dir.join("big/deep/blob"), vec![1; 1 << 20]).unwrap();
        fs::write(dir.join("small"), vec![1; 10]).unwrap();
        let list = analyze(&dir, &|| false);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "big");
        assert!(list[0].size >= 1 << 20 && list[0].dir);
        assert!(analyze(&dir, &|| true).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn installers_are_found_by_their_type() {
        let home = scratch("installers");
        fs::create_dir_all(home.join("Downloads/old")).unwrap();
        fs::write(home.join("Downloads/old/Tool.DMG"), "x").unwrap();
        fs::write(home.join("Downloads/notes.txt"), "x").unwrap();
        let list = installers(&home);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].note, "dmg");
        fs::remove_dir_all(&home).unwrap();
    }
}
