use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::gba::header_title_code;
use crate::platform::{Header, Platform};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cart {
    pub platform: Platform,
    pub stem: String,
    pub rom: PathBuf,
    pub label: Option<PathBuf>,
    pub title: String,
    pub code: String,
    pub shell: Option<crate::ShellChoice>,
}

impl Cart {
    pub fn read_header(&mut self) {
        match self.platform.spec().header {
            Header::Gba => {
                if let Some((title, code)) = header_title_code(&self.rom) {
                    self.title = title;
                    self.code = code;
                }
            }
            Header::GameBoy => {
                if let Some(title) = crate::gb::title(&self.rom) {
                    self.title = title;
                }
            }
            Header::None => {}
        }
    }
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub fn scan(root: &Path) -> Result<Vec<Cart>, StoreError> {
    let read = |file: &str| std::fs::read_to_string(root.join(file)).unwrap_or_default();
    let shells = crate::cart_shell::layered(
        &read(crate::CART_SHELL_FILE),
        &read(crate::LABELS_SHELL_FILE),
    );
    let mut carts = Vec::new();
    for platform in Platform::ALL {
        let dir = root.join("Games").join(platform.dir_name());
        let entries = match std::fs::read_dir(&dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("slot: scan: {}: {e}", dir.display());
                continue;
            }
        };
        let labels = Labels::read(root, platform);
        let entries: Vec<_> = entries.flatten().collect();
        let in_playlists = match platform.spec().playlists {
            true => playlist_members(&entries),
            false => HashSet::new(),
        };
        for entry in entries {
            let rom = entry.path();
            if is_hidden(&rom) || !platform.accepts(&rom) || !is_file(&entry) {
                continue;
            }
            if is_listed_by_playlist(&rom, &in_playlists) {
                continue;
            }
            let Some(stem) = rom.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let label = labels.find(stem);
            carts.push(Cart {
                platform,
                stem: stem.to_string(),
                title: String::new(),
                code: String::new(),
                shell: shells.get(&crate::cart_shell::key(stem)).copied(),
                label,
                rom,
            });
        }
    }
    carts.sort_by(|a, b| {
        (a.platform as u8, sort_key(&a.stem)).cmp(&(b.platform as u8, sort_key(&b.stem)))
    });
    Ok(carts)
}

/// The files a folder's `.m3u` playlists name, lower-cased and without any
/// directory part. A multi-disc game is shelved once, as its playlist, so its
/// individual discs must not appear beside it.
fn playlist_members(entries: &[std::fs::DirEntry]) -> HashSet<String> {
    let mut named = HashSet::new();
    for entry in entries {
        let path = entry.path();
        let is_m3u = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("m3u"));
        if !is_m3u || is_hidden(&path) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Playlists written on Windows use backslashes.
            let file = line.rsplit(['/', '\\']).next().unwrap_or(line);
            named.insert(file.to_lowercase());
        }
    }
    named
}

fn is_listed_by_playlist(rom: &Path, listed: &HashSet<String>) -> bool {
    let is_m3u = rom
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("m3u"));
    !is_m3u
        && rom
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| listed.contains(&n.to_lowercase()))
}

pub fn listing(dir: &Path) -> HashMap<String, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return HashMap::new();
    };
    entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .map(|name| (name.to_lowercase(), name))
        .collect()
}

/// The PNGs a cart of one system may use for its label.
///
/// People add these by hand on a computer, so the lookup forgives the two slips
/// that cost artwork with no explanation: dropping every image in one folder
/// instead of the per-system one, and naming it after the game rather than
/// after the ROM file. Nothing pairs on a guess beyond that; a label either
/// names the game or it does not.
struct Labels {
    files: Vec<(String, PathBuf)>,
}

impl Labels {
    fn read(root: &Path, platform: Platform) -> Labels {
        let base = root.join("Labels");
        // The system's own folder wins, so one image can be overridden.
        let dirs = [find_dir(&base, platform.dir_name()), Some(base)];
        let mut files: Vec<(String, PathBuf)> = Vec::new();
        for dir in dirs.into_iter().flatten() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(name) = entry.file_name().into_string() else {
                    continue;
                };
                let name = name.to_lowercase();
                if !name.ends_with(".png") || !is_file(&entry) {
                    continue;
                }
                if files.iter().any(|(seen, _)| *seen == name) {
                    continue;
                }
                files.push((name, entry.path()));
            }
        }
        Labels { files }
    }

    /// The image for a ROM: its exact name first, then one named after the game.
    fn find(&self, stem: &str) -> Option<PathBuf> {
        let exact = format!("{}.png", stem.to_lowercase());
        if let Some((_, path)) = self.files.iter().find(|(name, _)| *name == exact) {
            return Some(path.clone());
        }
        let game = tidy(stem);
        if game.is_empty() {
            return None;
        }
        self.files
            .iter()
            .find(|(name, _)| tidy(name.trim_end_matches(".png")) == game)
            .map(|(_, path)| path.clone())
    }
}

/// `Labels/GBA` under whatever case the folder was made: a card written on
/// Windows or by hand often has `gba`.
fn find_dir(base: &Path, name: &str) -> Option<PathBuf> {
    let want = base.join(name);
    if want.is_dir() {
        return Some(want);
    }
    std::fs::read_dir(base)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
}

/// A name reduced to the game it is about: case, region and revision tags in
/// brackets, and punctuation are dropped, so `Kirby's Dream Land (USA, Europe)`
/// and `Kirbys Dream Land` are the same game. Curly quotes come along free,
/// which matters because artwork copied from the web often has them.
fn tidy(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

fn is_file(entry: &std::fs::DirEntry) -> bool {
    match entry.file_type() {
        Ok(t) if t.is_symlink() => entry.path().is_file(),
        Ok(t) => t.is_file(),
        Err(_) => false,
    }
}

pub fn sort_key(stem: &str) -> (u8, String) {
    (group_of(stem), stem.to_uppercase())
}

fn group_of(stem: &str) -> u8 {
    match stem.chars().find(|c| !c.is_whitespace()) {
        Some(c) if c.is_ascii_digit() => 0,
        Some(c) if c.is_alphabetic() => 1,
        _ => 2,
    }
}

pub fn initial(stem: &str) -> char {
    match group_of(stem) {
        1 => stem
            .chars()
            .find(|c| !c.is_whitespace())
            .and_then(|c| c.to_uppercase().next())
            .unwrap_or('#'),
        _ => '#',
    }
}

pub fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}
