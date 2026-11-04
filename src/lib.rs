//! The photo album: a folder of pictures and videos as a grid.
//!
//! It only reads. Subfolders are rows you enter with Right, and each folder's pictures
//! and videos are one row, a grid the app's in element mode moves around. Keys, the big
//! view and playing a video are all the app's: this plugin only says what is there.
//!
//! [`Album`] is the logic, and [`PhotoAlbum`] puts the host in front of it. A folder is
//! read on a thread of its own, because the app waits for every call and a folder on a
//! slow disk must not freeze its window.

use sicompass_sdk::ffon::FfonElement;
use sicompass_sdk::markup::escape_attribute;
use sicompass_sdk::plugin::{Descriptor, Plugin, PollResult, host};
use sicompass_sdk::tags;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub const PLUGIN_NAME: &str = "photoalbum";

/// The album's stylesheet, sent as a `<style>` row at the top of each list.
const ALBUM_CSS: &str = include_str!("album.css");

const PICTURE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "mkv", "mov"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Name,
    Date,
}

impl Sort {
    pub fn from_setting(value: &str) -> Sort {
        if value == "chronologically" {
            Sort::Date
        } else {
            Sort::Name
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    Picture,
    Video,
}

/// What a file's name says it is, or nothing for a file the album skips.
pub fn media_kind(name: &str) -> Option<Media> {
    let extension = Path::new(name).extension()?.to_str()?.to_lowercase();
    if PICTURE_EXTENSIONS.contains(&extension.as_str()) {
        Some(Media::Picture)
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        Some(Media::Video)
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub path: PathBuf,
    pub media: Media,
    modified: Option<SystemTime>,
}

/// One folder, read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub folders: Vec<String>,
    pub items: Vec<Item>,
}

/// Compare names the way a person reads them: `IMG_2` before `IMG_10`, and upper and lower
/// case alike. A run of digits compares as a number, however long it is.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a_low, b_low) = (a.to_lowercase(), b.to_lowercase());
    let (mut x, mut y) = (a_low.as_str(), b_low.as_str());
    while let (Some(cx), Some(cy)) = (x.chars().next(), y.chars().next()) {
        let order = if cx.is_ascii_digit() && cy.is_ascii_digit() {
            let (nx, rest_x) = split_number(x);
            let (ny, rest_y) = split_number(y);
            x = rest_x;
            y = rest_y;
            nx.len().cmp(&ny.len()).then_with(|| nx.cmp(ny))
        } else {
            x = &x[cx.len_utf8()..];
            y = &y[cy.len_utf8()..];
            cx.cmp(&cy)
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    // One ran out first, or they read the same, and then the order is still the same each
    // time: `007` next to `7`, `A` next to `a`.
    x.len().cmp(&y.len()).then_with(|| a.cmp(b))
}

/// The digits at the start of `text`, without leading zeros, and what follows them.
fn split_number(text: &str) -> (&str, &str) {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, rest) = text.split_at(end);
    (digits.trim_start_matches('0'), rest)
}

/// Read a folder. Hidden files and anything that is not a picture or a video are skipped.
pub fn read_folder(dir: &Path, sort: Sort) -> std::io::Result<Listing> {
    let mut folders: Vec<(String, Option<SystemTime>)> = Vec::new();
    let mut items = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        // Follows links, so a linked folder of photos is a folder here too.
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        let modified = metadata.modified().ok();
        if metadata.is_dir() {
            folders.push((name, modified));
        } else if let Some(media) = media_kind(&name) {
            items.push(Item {
                name,
                path,
                media,
                modified,
            });
        }
    }
    let by_name = natural_cmp;
    match sort {
        Sort::Name => {
            folders.sort_by(|a, b| by_name(&a.0, &b.0));
            items.sort_by(|a, b| by_name(&a.name, &b.name));
        }
        // Oldest first, the order they were taken in.
        Sort::Date => {
            folders.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| by_name(&a.0, &b.0)));
            items.sort_by(|a, b| {
                a.modified
                    .cmp(&b.modified)
                    .then_with(|| by_name(&a.name, &b.name))
            });
        }
    }
    Ok(Listing {
        folders: folders.into_iter().map(|(name, _)| name).collect(),
        items,
    })
}

/// The row that holds a folder's pictures and videos.
///
/// Every file name is escaped here, because a plugin that asks for markup answers for
/// what it puts in it: a file called `<b>x.jpg` is a file, not a tag.
pub fn grid_row(items: &[Item]) -> String {
    let mut row = String::from(r#"<div class="grid-container">"#);
    for item in items {
        let path = escape_attribute(&item.path.to_string_lossy());
        let text = tags::escape(&item.name);
        row.push_str(r#"<div class="grid-item">"#);
        match item.media {
            Media::Picture => {
                let alt = escape_attribute(&item.name);
                row.push_str(&format!(r#"<img src="{path}" alt="{alt}"><p>{text}</p>"#));
            }
            Media::Video => row.push_str(&format!(
                r#"<video src="{path}">{text}</video><p>{text}</p>"#
            )),
        }
        row.push_str("</div>");
    }
    row.push_str("</div>");
    row
}

/// The lines the album says itself, so the tests need no host.
#[derive(Clone, Debug, Default)]
pub struct Texts {
    pub loading: String,
    pub empty: String,
    /// `%s` is the folder.
    pub unreadable: String,
}

impl Texts {
    fn from_host() -> Texts {
        Texts {
            loading: host::translate("photoalbum-loading"),
            empty: host::translate("photoalbum-empty"),
            unreadable: host::translate("photoalbum-unreadable"),
        }
    }
}

/// The rows of one read folder.
pub fn rows(listing: &Listing, texts: &Texts) -> Vec<FfonElement> {
    let mut out = vec![FfonElement::Str(format!("<style>{ALBUM_CSS}</style>"))];
    for folder in &listing.folders {
        out.push(FfonElement::new_obj(tags::escape(folder)));
    }
    if !listing.items.is_empty() {
        out.push(FfonElement::Str(grid_row(&listing.items)));
    } else if listing.folders.is_empty() {
        out.push(FfonElement::Str(tags::escape(&texts.empty)));
    }
    out
}

/// The folder a setting names: as written when absolute, under the home folder otherwise.
pub fn resolve_folder(setting: &str, home: Option<&Path>) -> PathBuf {
    let setting = setting.trim();
    let setting = if setting.is_empty() {
        "Pictures"
    } else {
        setting
    };
    let under_home = |rest: &str| match home {
        Some(home) => home.join(rest),
        None => PathBuf::from(rest),
    };
    if let Some(rest) = setting.strip_prefix("~/") {
        under_home(rest)
    } else if Path::new(setting).is_absolute() {
        PathBuf::from(setting)
    } else {
        under_home(setting)
    }
}

/// A finished read, with which request it answers.
struct Finished {
    request: u64,
    result: Result<Listing, String>,
}

pub struct Album {
    root: PathBuf,
    /// `/`, or the path below `root`, like `/2026/summer`.
    current: String,
    sort: Sort,
    texts: Texts,
    /// What is known about the current folder.
    listing: Option<Result<Listing, String>>,
    /// The latest read asked for. An older one's answer is about somewhere else.
    request: u64,
    reading: bool,
    finished: Arc<Mutex<Option<Finished>>>,
}

impl Album {
    pub fn new(root: PathBuf, sort: Sort, texts: Texts) -> Album {
        Album {
            root,
            current: "/".to_owned(),
            sort,
            texts,
            listing: None,
            request: 0,
            reading: false,
            finished: Arc::new(Mutex::new(None)),
        }
    }

    pub fn current_path(&self) -> &str {
        &self.current
    }

    /// The folder on disk the current path means.
    pub fn current_folder(&self) -> PathBuf {
        let mut folder = self.root.clone();
        for segment in self.current.split('/').filter(|s| !s.is_empty()) {
            folder.push(segment);
        }
        folder
    }

    /// Forget the current folder, so the next fetch reads it again.
    fn invalidate(&mut self) {
        self.listing = None;
        self.reading = false;
        self.request += 1;
    }

    pub fn set_current_path(&mut self, path: &str) {
        self.current = if path.is_empty() {
            "/".to_owned()
        } else {
            path.to_owned()
        };
        self.invalidate();
    }

    pub fn push_path(&mut self, segment: &str) {
        if !self.current.ends_with('/') {
            self.current.push('/');
        }
        self.current.push_str(segment);
        self.invalidate();
    }

    pub fn pop_path(&mut self) {
        match self.current.rfind('/') {
            Some(0) | None => self.current = "/".to_owned(),
            Some(at) => self.current.truncate(at),
        }
        self.invalidate();
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = root;
        self.current = "/".to_owned();
        self.invalidate();
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.invalidate();
    }

    fn start_reading(&mut self) {
        self.reading = true;
        let request = self.request;
        let folder = self.current_folder();
        let sort = self.sort;
        let finished = Arc::clone(&self.finished);
        std::thread::spawn(move || {
            let result = read_folder(&folder, sort).map_err(|e| e.to_string());
            if let Ok(mut slot) = finished.lock() {
                // A slow read of the folder you just left may end last. It must not
                // overwrite the answer the album is waiting for.
                if slot.as_ref().is_none_or(|newer| newer.request < request) {
                    *slot = Some(Finished { request, result });
                }
            }
        });
    }

    /// True once, when a read of the current folder has finished, or when the album was
    /// told to look again and the rows on screen are out of date.
    pub fn take_news(&mut self) -> bool {
        let Ok(mut slot) = self.finished.lock() else {
            return false;
        };
        let Some(finished) = slot.take() else {
            return false;
        };
        if finished.request != self.request {
            return false;
        }
        self.reading = false;
        self.listing = Some(finished.result);
        true
    }

    /// The rows of the current folder, or a line saying they are on their way.
    pub fn fetch(&mut self) -> Vec<FfonElement> {
        self.take_news();
        match &self.listing {
            Some(Ok(listing)) => rows(listing, &self.texts),
            Some(Err(_)) => {
                let folder = self.current_folder().to_string_lossy().into_owned();
                let line = self.texts.unreadable.replace("%s", &folder);
                vec![FfonElement::Str(tags::escape(&line))]
            }
            None => {
                if !self.reading {
                    self.start_reading();
                }
                vec![FfonElement::Str(tags::escape(&self.texts.loading))]
            }
        }
    }
}

/// The photo album as the host sees it.
pub struct PhotoAlbum {
    album: Album,
    /// A setting changed, so the rows on screen are out of date.
    changed: bool,
}

fn folder_from_setting(value: &str) -> PathBuf {
    resolve_folder(value, std::env::home_dir().as_deref())
}

impl Plugin for PhotoAlbum {
    fn new() -> Self {
        PhotoAlbum {
            album: Album::new(folder_from_setting(""), Sort::Name, Texts::default()),
            changed: false,
        }
    }

    fn describe(&self) -> Descriptor {
        Descriptor {
            name: PLUGIN_NAME.to_owned(),
            display_name: host::translate("photoalbum-display-name"),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            // A folder is read again each time you go into it.
            no_cache: true,
            ..Default::default()
        }
    }

    fn init(&mut self) {
        self.album.texts = Texts::from_host();
        if let Some(folder) = host::get_setting("photoFolder") {
            self.album.set_root(folder_from_setting(&folder));
        }
        if let Some(sort) = host::get_setting("sortOrder") {
            self.album.set_sort(Sort::from_setting(&sort));
        }
    }

    fn poll(&mut self) -> PollResult {
        let path = self.album.current_path();
        let at_root = path.is_empty() || path == "/";
        let news = self.album.take_news() || std::mem::take(&mut self.changed);
        PollResult {
            at_root,
            // `redraw` is what makes the app look at this tick at all, and
            // `needs_refresh` is what it does then. Without the first a folder you
            // just went into stays on the loading line.
            redraw: news,
            needs_refresh: news,
            structural_edit_here: false,
            dashboard_here: false,
            ..Default::default()
        }
    }

    fn fetch(&mut self) -> Vec<FfonElement> {
        self.album.fetch()
    }

    fn current_path(&self) -> &str {
        self.album.current_path()
    }

    fn set_current_path(&mut self, path: &str) {
        self.album.set_current_path(path);
    }

    fn push_path(&mut self, segment: &str) {
        self.album.push_path(segment);
    }

    fn pop_path(&mut self) {
        self.album.pop_path();
    }

    fn on_setting_change(&mut self, key: &str, value: &str) {
        match key {
            "photoFolder" => self.album.set_root(folder_from_setting(value)),
            "sortOrder" => self.album.set_sort(Sort::from_setting(value)),
            _ => return,
        }
        self.changed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sicompass_sdk::markup;
    use std::time::{Duration, Instant};

    /// A folder for one test, gone when it ends.
    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Temp {
            let dir =
                std::env::temp_dir().join(format!("photoalbum-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }

        fn file(&self, name: &str) -> &Temp {
            std::fs::write(self.0.join(name), b"x").unwrap();
            self
        }

        fn folder(&self, name: &str) -> &Temp {
            std::fs::create_dir_all(self.0.join(name)).unwrap();
            self
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn texts() -> Texts {
        Texts {
            loading: "loading".to_owned(),
            empty: "nothing here".to_owned(),
            unreadable: "cannot read %s".to_owned(),
        }
    }

    fn album(root: &Path) -> Album {
        Album::new(root.to_path_buf(), Sort::Name, texts())
    }

    /// The rows once the read has finished, the way the app sees them: poll until there
    /// is news, then fetch.
    fn loaded(album: &mut Album) -> Vec<FfonElement> {
        album.fetch();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !album.take_news() {
            assert!(Instant::now() < deadline, "the read never finished");
            std::thread::sleep(Duration::from_millis(5));
        }
        album.fetch()
    }

    fn strs(rows: &[FfonElement]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                FfonElement::Str(s) => s.clone(),
                FfonElement::Obj(o) => format!("+{}", o.key),
            })
            .collect()
    }

    fn the_grid(rows: &[FfonElement]) -> String {
        strs(rows)
            .into_iter()
            .find(|r| r.starts_with("<div"))
            .expect("a grid row")
    }

    #[test]
    fn only_pictures_and_videos_count() {
        for name in ["a.jpg", "A.JPEG", "b.png", "c.webp"] {
            assert_eq!(media_kind(name), Some(Media::Picture), "{name}");
        }
        for name in ["a.mp4", "b.WEBM", "c.mkv", "d.mov"] {
            assert_eq!(media_kind(name), Some(Media::Video), "{name}");
        }
        for name in ["notes.txt", "raw.cr2", "jpg", "archive.tar.gz"] {
            assert_eq!(media_kind(name), None, "{name}");
        }
    }

    #[test]
    fn a_folder_is_subfolders_then_one_row_of_its_pictures_and_videos() {
        let t = Temp::new("tree");
        t.file("b.jpg")
            .file("a.png")
            .file("clip.mp4")
            .file("notes.txt")
            .file(".hidden.jpg");
        t.folder("Summer").folder("autumn").folder(".cache");
        let rows = loaded(&mut album(&t.0));
        let got = strs(&rows);
        assert!(got[0].starts_with("<style>") && got[0].contains("grid-template-columns"));
        assert_eq!(got[1].trim_end_matches(':'), "+autumn");
        assert_eq!(got[2].trim_end_matches(':'), "+Summer");
        assert_eq!(got.len(), 4, "{got:?}");

        let counts = markup::target_counts(&got[3]);
        assert_eq!(
            (counts.images, counts.videos, counts.total()),
            (2, 1, 3),
            "{}",
            got[3]
        );
        let targets = markup::targets(&got[3]);
        let order: Vec<&str> = targets
            .iter()
            .map(|t| t.value.rsplit('/').next().unwrap())
            .collect();
        assert_eq!(order, ["a.png", "b.jpg", "clip.mp4"]);
    }

    #[test]
    fn the_grid_is_the_shape_the_stylesheet_addresses() {
        let t = Temp::new("shape");
        t.file("a.jpg");
        let got = strs(&loaded(&mut album(&t.0)));
        assert!(
            got[1].starts_with(r#"<div class="grid-container"><div class="grid-item"><img "#),
            "{}",
            got[1]
        );
        assert!(ALBUM_CSS.contains(".grid-container") && ALBUM_CSS.contains("repeat(3, 1fr)"));
        assert!(ALBUM_CSS.contains("max-width: none"));
    }

    /// Asking for markup makes the file names this plugin's to escape.
    #[test]
    fn a_file_name_with_tags_in_it_stays_a_file_name() {
        let t = Temp::new("names");
        t.file("<b>x.jpg")
            .file(r#"say "hi" & go.png"#)
            .file("<video src=y>.mp4");
        let rows = loaded(&mut album(&t.0));
        let row = the_grid(&rows);
        let targets = markup::targets(&row);
        assert_eq!(targets.len(), 3, "{row}");
        let names: Vec<String> = targets
            .iter()
            .map(|t| t.value.rsplit('/').next().unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            ["<b>x.jpg", "<video src=y>.mp4", r#"say "hi" & go.png"#]
        );
        for target in &targets {
            assert!(
                Path::new(&target.value).is_file(),
                "{} is not the real path",
                target.value
            );
        }
        // Nothing in the row reads as bold or as a fourth target.
        assert!(
            markup::plain(&row).contains("<b>x.jpg"),
            "{}",
            markup::plain(&row)
        );
    }

    #[test]
    fn a_folder_with_tags_in_its_name_can_be_entered() {
        let t = Temp::new("tagfolder");
        t.folder("<i>trip");
        std::fs::write(t.0.join("<i>trip").join("a.jpg"), b"x").unwrap();
        let mut album = album(&t.0);
        let got = strs(&loaded(&mut album));
        // The app pushes what the row displays, with the escaping undone.
        let key = got[1].trim_start_matches('+').trim_end_matches(':');
        album.push_path(&tags::strip_display(key));
        assert_eq!(album.current_path(), "/<i>trip");
        let got = strs(&loaded(&mut album));
        assert_eq!(markup::target_counts(&got[1]).images, 1, "{got:?}");
    }

    #[test]
    fn an_empty_folder_says_so_and_a_missing_one_says_which() {
        let t = Temp::new("empty");
        let got = strs(&loaded(&mut album(&t.0)));
        assert_eq!(got[1], "nothing here");

        let gone = t.0.join("gone");
        let got = strs(&loaded(&mut album(&gone)));
        assert_eq!(got, [format!("cannot read {}", gone.display())]);
    }

    #[test]
    fn subfolders_with_no_pictures_beside_them_are_not_an_empty_folder() {
        let t = Temp::new("onlyfolders");
        t.folder("sub");
        let got = strs(&loaded(&mut album(&t.0)));
        assert_eq!(got.len(), 2, "{got:?}");
    }

    #[test]
    fn going_in_and_back_out_reads_the_right_folders() {
        let t = Temp::new("nav");
        t.folder("a");
        std::fs::create_dir_all(t.0.join("a/b")).unwrap();
        std::fs::write(t.0.join("a/b/deep.jpg"), b"x").unwrap();
        let mut album = album(&t.0);
        assert_eq!(album.current_path(), "/");
        album.push_path("a");
        album.push_path("b");
        assert_eq!(album.current_path(), "/a/b");
        assert_eq!(album.current_folder(), t.0.join("a").join("b"));
        assert!(the_grid(&loaded(&mut album)).contains("deep.jpg"));
        album.pop_path();
        assert_eq!(album.current_path(), "/a");
        album.pop_path();
        assert_eq!(album.current_path(), "/");
        album.pop_path();
        assert_eq!(album.current_path(), "/", "the top stays the top");
    }

    /// While a slow read is out, you may have gone elsewhere.
    #[test]
    fn a_read_of_where_you_were_is_not_shown_where_you_are() {
        let t = Temp::new("stale");
        t.file("top.jpg").folder("sub");
        std::fs::write(t.0.join("sub/inner.jpg"), b"x").unwrap();
        let mut album = album(&t.0);
        album.fetch();
        album.push_path("sub");
        // However the two reads finish, this folder's rows are this folder's.
        let row = the_grid(&loaded(&mut album));
        assert!(
            row.contains("inner.jpg") && !row.contains("top.jpg"),
            "{row}"
        );
    }

    #[test]
    fn the_first_fetch_does_not_wait_for_the_disk() {
        let t = Temp::new("fast");
        t.file("a.jpg");
        let mut album = album(&t.0);
        assert_eq!(strs(&album.fetch()), ["loading"]);
    }

    #[test]
    fn a_folder_is_read_again_each_time_you_go_into_it() {
        let t = Temp::new("again");
        t.folder("sub");
        let mut album = album(&t.0);
        album.push_path("sub");
        assert_eq!(strs(&loaded(&mut album))[1], "nothing here");
        album.pop_path();
        std::fs::write(t.0.join("sub/new.jpg"), b"x").unwrap();
        album.push_path("sub");
        assert!(the_grid(&loaded(&mut album)).contains("new.jpg"));
    }

    #[test]
    fn sorting_by_date_is_oldest_first() {
        let t = Temp::new("dates");
        t.file("z-old.jpg").file("a-new.jpg");
        let old = std::fs::File::options()
            .write(true)
            .open(t.0.join("z-old.jpg"))
            .unwrap();
        old.set_modified(SystemTime::now() - Duration::from_secs(86_400))
            .unwrap();
        let names = |sort| -> Vec<String> {
            read_folder(&t.0, sort)
                .unwrap()
                .items
                .into_iter()
                .map(|i| i.name)
                .collect()
        };
        assert_eq!(names(Sort::Date), ["z-old.jpg", "a-new.jpg"]);
        assert_eq!(names(Sort::Name), ["a-new.jpg", "z-old.jpg"]);
        assert_eq!(Sort::from_setting("chronologically"), Sort::Date);
        assert_eq!(Sort::from_setting("alphanumerically"), Sort::Name);
    }

    #[test]
    fn the_setting_means_a_folder_under_home_unless_it_says_otherwise() {
        let home = Path::new("/home/someone");
        assert_eq!(
            resolve_folder("Pictures", Some(home)),
            home.join("Pictures")
        );
        assert_eq!(
            resolve_folder("~/Photos/2026", Some(home)),
            home.join("Photos/2026")
        );
        assert_eq!(resolve_folder("  ", Some(home)), home.join("Pictures"));
        #[cfg(unix)]
        assert_eq!(
            resolve_folder("/mnt/photos", Some(home)),
            PathBuf::from("/mnt/photos")
        );
    }

    #[test]
    fn changing_the_folder_goes_back_to_its_top() {
        let t = Temp::new("setroot");
        t.file("here.jpg");
        let mut album = album(Path::new("/nowhere"));
        album.push_path("deep");
        album.set_root(t.0.clone());
        assert_eq!(album.current_path(), "/");
        assert!(the_grid(&loaded(&mut album)).contains("here.jpg"));
    }

    #[test]
    fn names_sort_the_way_a_person_reads_them() {
        let t = Temp::new("natural");
        t.file("IMG_10.jpg")
            .file("IMG_2.jpg")
            .file("img_3.jpg")
            .file("IMG_1.jpg");
        t.folder("Trip 10").folder("Trip 9");
        let listing = read_folder(&t.0, Sort::Name).unwrap();
        let names: Vec<&str> = listing.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["IMG_1.jpg", "IMG_2.jpg", "img_3.jpg", "IMG_10.jpg"]);
        assert_eq!(listing.folders, ["Trip 9", "Trip 10"]);
    }

    #[test]
    fn natural_order_holds_at_the_edges() {
        let before = |a, b| natural_cmp(a, b) == Ordering::Less;
        assert!(before("a2", "a10"));
        assert!(before("a", "a1"));
        assert!(before("2", "a"), "digits before letters");
        assert!(before("photo-119", "photo-120"));
        assert!(before("photo-12", "photo-119"));
        // Longer than any integer type.
        assert!(before(
            "99999999999999999999999",
            "100000000000000000000000"
        ));
        // Same reading, and still one fixed order, so a sort cannot wobble.
        assert_eq!(natural_cmp("007", "007"), Ordering::Equal);
        assert_ne!(natural_cmp("007", "7"), Ordering::Equal);
        assert_ne!(natural_cmp("A", "a"), Ordering::Equal);
        assert_eq!(natural_cmp("007", "7"), natural_cmp("7", "007").reverse());
        assert_eq!(natural_cmp("é1", "é2"), Ordering::Less);
    }

    /// The app rebuilds a list only for a tick that says `redraw`.
    #[test]
    fn a_finished_read_tells_the_app_to_redraw_and_refresh() {
        let t = Temp::new("poll");
        t.file("a.jpg");
        let mut plugin = PhotoAlbum {
            album: Album::new(t.0.clone(), Sort::Name, texts()),
            changed: false,
        };
        assert_eq!(strs(&plugin.fetch()), ["loading"]);
        let deadline = Instant::now() + Duration::from_secs(10);
        let news = loop {
            let polled = plugin.poll();
            if polled.redraw || polled.needs_refresh {
                break polled;
            }
            assert!(Instant::now() < deadline, "the read never finished");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(news.redraw && news.needs_refresh);
        assert!(!news.structural_edit_here && !news.dashboard_here);
        let quiet = plugin.poll();
        assert!(!quiet.redraw && !quiet.needs_refresh, "it says so once");
        assert!(the_grid(&plugin.fetch()).contains("a.jpg"));

        plugin.on_setting_change("sortOrder", "chronologically");
        let changed = plugin.poll();
        assert!(changed.redraw && changed.needs_refresh);
    }

    /// Every id the code or the manifest names exists, in every language.
    #[test]
    fn every_language_has_every_line() {
        let languages = [
            ("en-US", include_str!("../locales/en-US.ftl")),
            ("nl-BE", include_str!("../locales/nl-BE.ftl")),
            ("fr-BE", include_str!("../locales/fr-BE.ftl")),
            ("de-BE", include_str!("../locales/de-BE.ftl")),
        ];
        for id in [
            "photoalbum-display-name",
            "photoalbum-description",
            "photoalbum-tutorial",
            "photoalbum-loading",
            "photoalbum-empty",
            "photoalbum-unreadable",
            "photoalbum-radio-sort-order",
        ] {
            for (language, text) in languages {
                assert!(
                    text.contains(&format!("\n{id} = ")),
                    "{language} has no {id}"
                );
            }
        }
        for (language, text) in languages {
            assert!(
                text.contains("%s"),
                "{language} must keep the folder placeholder"
            );
        }
    }

    #[test]
    fn the_manifest_asks_for_markup() {
        let manifest = include_str!("../plugin.json");
        assert!(manifest.contains(r#""markup": true"#));
        assert!(manifest.contains(r#""displayName": "photo album""#));
        assert!(manifest.contains(&format!(r#""version": "{}""#, env!("CARGO_PKG_VERSION"))));
    }
}
