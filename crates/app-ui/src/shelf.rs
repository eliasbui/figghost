//! The shelf: files dropped on the notch, kept as references to where they live (never copied), shown as
//! a tab of the notch and remembered between runs.

use crate::{
    history::now_ms,
    i18n,
    shelf_actions::{self, Action},
};
use macos_sys::{
    MainThreadMarker, app, capture_ui,
    notch::Notch,
    ocr,
    pasteboard::{Pasteboard, Write},
    services::{self, Services},
    shelf_ui::{Drag, ShelfAction, ShelfCallbacks, ShelfView},
    workspace,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    env, fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    rc::Rc,
};

const LIMIT: usize = 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub added_ms: i64,
}

/// Newest first.
pub struct Shelf {
    entries: Vec<Entry>,
    file: PathBuf,
}

impl Shelf {
    /// A missing file means an empty shelf; an unreadable one is moved aside so a later save cannot
    /// destroy it. Files that no longer exist are dropped.
    pub fn load(dir: &Path) -> Self {
        let file = dir.join("shelf.json");
        let entries = match fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                let backup = file.with_extension("json.bad");
                let moved = fs::rename(&file, &backup).is_ok();
                eprintln!(
                    "shelf: {} is invalid ({e}){}",
                    file.display(),
                    if moved { ", moved aside" } else { "" }
                );
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        let mut shelf = Self { entries, file };
        shelf.prune_missing();
        shelf
    }

    /// Puts the paths at the front in the order given; a path already on the shelf just moves up.
    /// Returns how many paths were accepted. Missing and relative paths are ignored.
    pub fn add(&mut self, paths: Vec<String>, now: i64) -> usize {
        let accepted: Vec<String> = paths
            .into_iter()
            .filter(|p| Path::new(p).is_absolute() && Path::new(p).exists())
            .collect();
        for path in accepted.iter().rev() {
            self.entries.retain(|e| &e.path != path);
            self.entries.insert(
                0,
                Entry {
                    path: path.clone(),
                    added_ms: now,
                },
            );
        }
        self.entries.truncate(LIMIT);
        accepted.len()
    }

    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.path != path);
        self.entries.len() != before
    }

    /// Drops entries whose file is gone; returns whether anything changed.
    pub fn prune_missing(&mut self) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| Path::new(&e.path).exists());
        self.entries.len() != before
    }

    pub fn paths(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.path.clone()).collect()
    }

    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_vec_pretty(&self.entries).map_err(io::Error::other)?,
        )?;
        fs::rename(tmp, &self.file)
    }
}

type AddedHook = Box<dyn Fn(&[String])>;

/// Keeps the shelf and its tab alive.
pub struct ShelfFeature {
    view: Rc<ShelfView>,
    shelf: Rc<RefCell<Shelf>>,
    refresh: Rc<dyn Fn()>,
    /// "Add to Figghost Shelf" in the Services menu and in Finder's right click menu.
    _services: Services,
    /// Told the files that came in through the Services menu, once they are on the shelf.
    on_added: Rc<RefCell<Option<AddedHook>>>,
}

impl ShelfFeature {
    pub fn attach(mtm: MainThreadMarker, notch: &Notch, home: &Path) -> Self {
        let shelf = Rc::new(RefCell::new(Shelf::load(home)));
        // The callbacks refresh the view they belong to, which does not exist until they have been built.
        let view_slot: Rc<RefCell<Option<Rc<ShelfView>>>> = Rc::default();

        let refresh: Rc<dyn Fn()> = {
            let (shelf, view_slot) = (Rc::clone(&shelf), Rc::clone(&view_slot));
            Rc::new(move || {
                let mut shelf = shelf.borrow_mut();
                if shelf.prune_missing() {
                    save(&shelf);
                }
                if let Some(view) = view_slot.borrow().as_ref() {
                    view.set_paths(shelf.paths());
                }
            })
        };
        let on_added: Rc<RefCell<Option<AddedHook>>> = Rc::default();
        let callbacks = {
            let (on_drop_shelf, on_drop_refresh, on_drop_added) =
                (Rc::clone(&shelf), Rc::clone(&refresh), Rc::clone(&on_added));
            let (on_remove_shelf, on_remove_refresh) = (Rc::clone(&shelf), Rc::clone(&refresh));
            let (action_shelf, action_refresh) = (Rc::clone(&shelf), Rc::clone(&refresh));
            ShelfCallbacks {
                on_drop: Box::new(move |paths| {
                    {
                        let mut shelf = on_drop_shelf.borrow_mut();
                        shelf.add(paths.clone(), now_ms());
                        save(&shelf);
                    }
                    on_drop_refresh();
                    if let Some(hook) = &*on_drop_added.borrow() {
                        hook(&paths);
                    }
                }),
                on_remove: Box::new(move |path| {
                    {
                        let mut shelf = on_remove_shelf.borrow_mut();
                        shelf.remove(&path);
                        save(&shelf);
                    }
                    on_remove_refresh();
                }),
                on_open: Box::new(|path| workspace::open_path(Path::new(&path))),
                actions_for: Box::new(menu_for),
                on_action: Box::new(move |path, id| match Action::from_id(id) {
                    Some(Action::Remove) => {
                        {
                            let mut shelf = action_shelf.borrow_mut();
                            shelf.remove(path);
                            save(&shelf);
                        }
                        action_refresh();
                    }
                    Some(Action::Reveal) => workspace::reveal(Path::new(path)),
                    Some(Action::CopyText) => start_text_copy(path),
                    Some(other) => start_job(other, path),
                    None => {}
                }),
            }
        };

        let view = ShelfView::new(mtm, notch.page_frame(), callbacks);
        *view_slot.borrow_mut() = Some(Rc::clone(&view));
        let on_show = Rc::clone(&refresh);
        notch.add_page(view.page(move || on_show()));
        {
            let (shelf, refresh) = (Rc::clone(&shelf), Rc::clone(&refresh));
            ADD_FROM_JOB.with(|hook| {
                *hook.borrow_mut() = Some(Rc::new(move |path| {
                    {
                        let mut shelf = shelf.borrow_mut();
                        shelf.add(vec![path], now_ms());
                        save(&shelf);
                    }
                    refresh();
                }));
            });
        }
        refresh();
        let services = {
            let (shelf, refresh, on_added) =
                (Rc::clone(&shelf), Rc::clone(&refresh), Rc::clone(&on_added));
            services::provide(mtm, move |paths| {
                {
                    let mut shelf = shelf.borrow_mut();
                    shelf.add(paths.clone(), now_ms());
                    save(&shelf);
                }
                refresh();
                if let Some(hook) = &*on_added.borrow() {
                    hook(&paths);
                }
            })
        };
        Self {
            view,
            shelf,
            refresh,
            _services: services,
            on_added,
        }
    }

    /// Puts files on the shelf, as dropping them on the notch does; returns how many were accepted.
    pub fn add_files(&self, paths: &[PathBuf]) -> usize {
        let paths: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let added = {
            let mut shelf = self.shelf.borrow_mut();
            let added = shelf.add(paths.clone(), now_ms());
            save(&shelf);
            added
        };
        (self.refresh)();
        if let Some(hook) = &*self.on_added.borrow() {
            hook(&paths);
        }
        added
    }

    /// Told the files that came into the shelf, after they are on the shelf.
    pub fn on_added(&self, hook: impl Fn(&[String]) + 'static) {
        *self.on_added.borrow_mut() = Some(Box::new(hook));
    }

    /// Told where a file dragged out of the shelf is, from the start of the drag to its end.
    pub fn on_drag(&self, hook: impl Fn(Drag) + 'static) {
        self.view.set_on_drag(hook);
    }

    /// Files on the shelf.
    pub fn count(&self) -> usize {
        self.shelf.borrow().paths().len()
    }

    /// Empties the shelf. The files themselves are never touched: the shelf only holds references.
    pub fn clear(&self) {
        {
            let mut shelf = self.shelf.borrow_mut();
            for path in shelf.paths() {
                shelf.remove(&path);
            }
            save(&shelf);
        }
        (self.refresh)();
    }
}

type AddFile = Rc<dyn Fn(String)>;

thread_local! {
    /// Adds a file the shelf made (an archive, a converted image) once its job is done on another thread.
    static ADD_FROM_JOB: RefCell<Option<AddFile>> = const { RefCell::new(None) };
}

/// The menu a file offers, with titles in the app's language.
fn menu_for(path: &str) -> Vec<ShelfAction> {
    let language = i18n::current();
    let mut entries = Vec::new();
    for action in shelf_actions::available(Path::new(path)) {
        let title = match action {
            Action::Zip => language.shelf_menu_zip().to_owned(),
            Action::ToPng => language.shelf_menu_convert("PNG"),
            Action::ToJpeg => language.shelf_menu_convert("JPEG"),
            Action::ToHeic => language.shelf_menu_convert("HEIC"),
            Action::RemoveBackground => language.vision_remove_background().to_owned(),
            Action::Half => language.shelf_menu_half().to_owned(),
            Action::Quarter => language.shelf_menu_quarter().to_owned(),
            Action::CopyText => language.shelf_menu_copy_text().to_owned(),
            Action::Reveal => language.shelf_menu_reveal().to_owned(),
            Action::Remove => language.shelf_menu_remove().to_owned(),
        };
        if action == Action::Reveal {
            entries.push(ShelfAction::separator());
        }
        entries.push(ShelfAction::new(action.id(), &title));
    }
    entries
}

/// Runs a file job on a background thread; the new file joins the shelf when it is done.
fn start_job(action: Action, path: &str) {
    let source = PathBuf::from(path);
    std::thread::spawn(move || match shelf_actions::make(action, &source) {
        Ok(output) => {
            let output = output.to_string_lossy().into_owned();
            app::on_main(move || {
                let add = ADD_FROM_JOB.with(|hook| hook.borrow().clone());
                if let Some(add) = add {
                    add(output);
                }
            });
        }
        Err(e) => {
            eprintln!("shelf: {e}");
            app::on_main(workspace::beep);
        }
    });
}

/// A named pasteboard keeps tests away from the user's real clipboard, as the history does.
pub(crate) fn pasteboard() -> Pasteboard {
    match env::var("FIGGHOST_PASTEBOARD") {
        Ok(name) => Pasteboard::named(&name),
        Err(_) => Pasteboard::general(),
    }
}

/// Reads the text of an image: the file is read on a worker thread, the picture is recognised on the main
/// thread, where the answer comes back, and the text is copied and shown in a toast, as a capture with
/// text recognition does.
pub(crate) fn start_text_copy(path: &str) {
    let source = PathBuf::from(path);
    std::thread::spawn(move || {
        let shot = shelf_actions::load_image(&source);
        app::on_main(move || {
            // Back on the main thread, where the recognition and the toast live.
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let shot = match shot {
                Ok(shot) => shot,
                Err(e) => {
                    eprintln!("shelf: {e}");
                    capture_ui::toast(mtm, &i18n::current().shelf_image_failed(&e));
                    return;
                }
            };
            ocr::recognize(&shot, move |result| {
                let text = i18n::current();
                match result {
                    Ok(found) if !found.is_empty() => {
                        let mut parts = found.lines.clone();
                        parts.extend(found.codes.iter().cloned());
                        pasteboard().write(Write::Text(&parts.join("\n")));
                        capture_ui::toast(mtm, &text.shelf_text_copied(parts.len()));
                    }
                    Ok(_) => capture_ui::toast(mtm, text.shelf_no_text()),
                    Err(e) => capture_ui::toast(mtm, &text.shelf_image_failed(&e)),
                }
            });
        });
    });
}

fn save(shelf: &Shelf) {
    if let Err(e) = shelf.save() {
        eprintln!("shelf: cannot save: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "figghost-shelf-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, name: &str) -> String {
            let path = self.0.join(name);
            fs::write(&path, name).unwrap();
            path.to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn newest_first_and_a_repeated_file_moves_up_instead_of_duplicating() {
        let dir = TempDir::new();
        let (a, b, c) = (dir.file("a.txt"), dir.file("b.txt"), dir.file("c.txt"));
        let mut shelf = Shelf::load(&dir.0);
        assert_eq!(shelf.add(vec![a.clone(), b.clone()], 1), 2);
        assert_eq!(
            shelf.paths(),
            [a.clone(), b.clone()],
            "files dropped together keep their order"
        );
        shelf.add(vec![c.clone()], 2);
        assert_eq!(shelf.paths(), [c.clone(), a.clone(), b.clone()]);
        shelf.add(vec![b.clone()], 3);
        assert_eq!(shelf.paths(), [b, c, a]);
    }

    #[test]
    fn missing_and_relative_paths_are_ignored() {
        let dir = TempDir::new();
        let mut shelf = Shelf::load(&dir.0);
        let real = dir.file("real.txt");
        let ghost = dir.0.join("ghost.txt").to_string_lossy().into_owned();
        assert_eq!(
            shelf.add(vec![ghost, "relative.txt".into(), real.clone()], 1),
            1
        );
        assert_eq!(shelf.paths(), [real]);
    }

    #[test]
    fn the_shelf_is_capped_and_drops_its_oldest_files() {
        let dir = TempDir::new();
        let mut shelf = Shelf::load(&dir.0);
        let files: Vec<String> = (0..LIMIT + 5)
            .map(|i| dir.file(&format!("f{i}.txt")))
            .collect();
        for (i, f) in files.iter().enumerate() {
            shelf.add(vec![f.clone()], i as i64);
        }
        assert_eq!(shelf.paths().len(), LIMIT);
        assert_eq!(shelf.paths()[0], files[LIMIT + 4]);
        assert!(!shelf.paths().contains(&files[0]));
    }

    #[test]
    fn files_deleted_behind_our_back_disappear() {
        let dir = TempDir::new();
        let (keep, gone) = (dir.file("keep.txt"), dir.file("gone.txt"));
        let mut shelf = Shelf::load(&dir.0);
        shelf.add(vec![keep.clone(), gone.clone()], 1);
        shelf.save().unwrap();
        fs::remove_file(&gone).unwrap();
        assert!(shelf.prune_missing());
        assert_eq!(shelf.paths(), std::slice::from_ref(&keep));
        // And on the next launch too.
        shelf.add(vec![gone.clone()], 2);
        assert_eq!(Shelf::load(&dir.0).paths(), [keep]);
    }

    #[test]
    fn persistence_round_trips_and_remove_reports_what_it_did() {
        let dir = TempDir::new();
        let (a, b) = (dir.file("a.txt"), dir.file("b.txt"));
        let mut shelf = Shelf::load(&dir.0);
        shelf.add(vec![a.clone(), b.clone()], 7);
        shelf.save().unwrap();
        let mut again = Shelf::load(&dir.0);
        assert_eq!(again.paths(), [a.clone(), b.clone()]);
        assert!(again.remove(&a));
        assert!(!again.remove(&a));
        assert_eq!(again.paths(), [b]);
    }

    #[test]
    fn a_broken_file_is_moved_aside_not_overwritten() {
        let dir = TempDir::new();
        fs::write(dir.0.join("shelf.json"), "{ not json").unwrap();
        let shelf = Shelf::load(&dir.0);
        assert!(shelf.paths().is_empty());
        assert_eq!(
            fs::read_to_string(dir.0.join("shelf.json.bad")).unwrap(),
            "{ not json"
        );
    }
}
