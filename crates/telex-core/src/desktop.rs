//! How Telex lives on the desktop: the tray icon's words (unread Inbox mail per account), the
//! taskbar button's unread badge drawn as a small picture, notifications paused for a while,
//! and starting with the system (Windows' Run key for this user, an XDG autostart file on
//! Linux). The app's tray and window code (src-tauri's tray.rs) only puts these on screen.

use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDateTime, NaiveTime};

use crate::list::FolderTree;
use crate::{trn, FolderKind, LOCAL_ID};

/// One account's unread Inbox mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxUnread {
    pub email: String,
    pub unread: usize,
}

/// The unread mail in each server account's Inbox, in the tree's order (the folders on this
/// PC have no Inbox). What the tray icon, its tooltip and the taskbar badge count.
pub fn inbox_unread(tree: &FolderTree) -> Vec<InboxUnread> {
    tree.accounts
        .iter()
        .filter(|a| a.id != LOCAL_ID)
        .filter_map(|a| {
            let inbox = a.folder_of_kind(FolderKind::Inbox)?;
            let unread = tree
                .counts
                .get(&a.folder_key(&inbox.id))
                .map_or(0, |c| c.unread);
            Some(InboxUnread {
                email: a.email.clone(),
                unread,
            })
        })
        .collect()
}

/// "3 unread messages": the line of the tray's menu, and the tooltip's first line.
pub fn unread_line(n: usize) -> String {
    trn!(
        n,
        "{n} unread message",
        "{n} unread messages",
        "{n} nieprzeczytana wiadomość",
        "{n} nieprzeczytane wiadomości",
        "{n} nieprzeczytanych wiadomości"
    )
}

/// Windows cuts a tray tooltip at 127 characters (UTF-16 units).
pub const TOOLTIP_MOST: usize = 127;

/// The tray icon's tooltip: "Telex", how many unread messages wait in the Inboxes, then an
/// account a line ("anna@firma.pl: 3") as far as Windows shows it.
pub fn tooltip(unread: &[InboxUnread]) -> String {
    let total: usize = unread.iter().map(|u| u.unread).sum();
    if total == 0 {
        return "Telex".to_owned();
    }
    let mut text = format!("Telex · {}", unread_line(total));
    let with_mail: Vec<&InboxUnread> = unread.iter().filter(|u| u.unread > 0).collect();
    if with_mail.len() < 2 {
        return cut(text, TOOLTIP_MOST);
    }
    for (i, u) in with_mail.iter().enumerate() {
        let line = format!("\n{}: {}", u.email, u.unread);
        // Room for "\n…" if this isn't the last line.
        let room = if i + 1 == with_mail.len() { 0 } else { 2 };
        if utf16_len(&text) + utf16_len(&line) + room > TOOLTIP_MOST {
            text.push_str("\n…");
            break;
        }
        text.push_str(&line);
    }
    cut(text, TOOLTIP_MOST)
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn cut(text: String, most: usize) -> String {
    if utf16_len(&text) <= most {
        return text;
    }
    let mut out = String::new();
    for c in text.chars() {
        if utf16_len(&out) + c.len_utf16() + 1 > most {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// A PNG file (the tray icons the app carries) as RGBA pixels, with its width and height.
pub fn png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), width, height))
}

/// How long the tray's Pause notifications lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pause {
    /// An hour from now.
    Hour,
    /// Until the next 08:00 (the morning Send later knows).
    Morning,
    /// Until the user turns them back on (or Telex starts again).
    Off,
}

/// When notifications paused now come back; `None`: not by themselves.
pub fn pause_until(pause: Pause, now: NaiveDateTime) -> Option<NaiveDateTime> {
    match pause {
        Pause::Hour => Some(now + Duration::hours(1)),
        Pause::Morning => {
            let morning = NaiveTime::from_hms_opt(crate::later::MORNING, 0, 0)?;
            let today = now.date().and_time(morning);
            Some(if today > now {
                today
            } else {
                today + Duration::days(1)
            })
        }
        Pause::Off => None,
    }
}

/// The taskbar badge's color: Windows' own red for things that want attention, under white
/// figures (5.4:1).
const BADGE: [u8; 3] = [0xc4, 0x2b, 0x1c];

/// Figures 3 pixels wide and 5 high, a row a byte (the low three bits, left to right).
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 1, 1],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        _ => [0, 2, 7, 2, 0], // '+'
    }
}

/// What the badge says: the count, "99+" above 99.
pub fn badge_text(count: usize) -> String {
    if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    }
}

/// The taskbar button's overlay: the unread count in white on a red circle (one figure) or a
/// rounded square (more), `size` pixels square, RGBA. Drawn at the size Windows shows it
/// (16 px at 100 %), so the figures stay sharp: they are whole pixels, only the edge is
/// smoothed.
pub fn badge(count: usize, size: u32) -> Vec<u8> {
    let size = size.clamp(8, 256);
    let text = badge_text(count);
    let chars = text.chars().count() as u32;
    let n = size as usize;
    let mut rgba = vec![0u8; n * n * 4];
    // The shape: a circle for one figure, else a square with round corners.
    let radius = if chars == 1 {
        size as f32 / 2.0
    } else {
        size as f32 * 0.3
    };
    let edge = size as f32;
    for y in 0..n {
        for x in 0..n {
            // 4 × 4 samples a pixel give a smooth edge.
            let mut inside = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                    if in_rounded_square(px, py, edge, radius) {
                        inside += 1;
                    }
                }
            }
            let at = (y * n + x) * 4;
            rgba[at..at + 3].copy_from_slice(&BADGE);
            rgba[at + 3] = (inside * 255 / 16) as u8;
        }
    }
    // The figures: as large as whole pixels allow, a pixel apart, in the middle.
    let room_w = if chars == 1 {
        size * 3 / 4
    } else {
        size.saturating_sub(2)
    };
    let room_h = size * 3 / 4;
    let width = |s: u32| 3 * s * chars + (chars - 1);
    let mut scale = 1;
    while width(scale + 1) <= room_w && 5 * (scale + 1) <= room_h {
        scale += 1;
    }
    let left = (size.saturating_sub(width(scale))) / 2;
    let top = (size.saturating_sub(5 * scale)) / 2;
    for (i, c) in text.chars().enumerate() {
        let x0 = left + i as u32 * (3 * scale + 1);
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..3u32 {
                if bits & (4 >> col) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = (x0 + col * scale + dx) as usize;
                        let y = (top + row as u32 * scale + dy) as usize;
                        if x < n && y < n {
                            let at = (y * n + x) * 4;
                            rgba[at..at + 4].copy_from_slice(&[255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
    rgba
}

fn in_rounded_square(x: f32, y: f32, size: f32, r: f32) -> bool {
    let cx = x.clamp(r, size - r);
    let cy = y.clamp(r, size - r);
    let (dx, dy) = (x - cx, y - cy);
    dx * dx + dy * dy <= r * r
}

/// The argument that starts Telex hidden in the tray (or minimized, without a tray icon):
/// what starting with the system passes.
pub const HIDDEN_ARG: &str = "--tray";

/// The name Telex starts with the system under: "Telex" for the released app, and the
/// identifier in parentheses for other builds, so a test build never takes its place.
pub fn entry_name(identifier: &str) -> String {
    if identifier == "app.telexmail.desktop" {
        "Telex".to_owned()
    } else {
        format!("Telex ({identifier})")
    }
}

/// The command Windows' Run key holds: the program in quotes, then `--tray` when it should
/// start hidden.
pub fn run_command(exe: &Path, hidden: bool) -> String {
    let mut line = format!("\"{}\"", exe.display());
    if hidden {
        line.push(' ');
        line.push_str(HIDDEN_ARG);
    }
    line
}

/// An XDG autostart file (freedesktop.org's Desktop Application Autostart): GNOME, KDE,
/// Xfce and the others start what is in ~/.config/autostart when the user logs in.
pub fn desktop_entry(name: &str, exe: &Path, hidden: bool) -> String {
    let mut exec = exec_arg(&exe.display().to_string());
    if hidden {
        exec.push(' ');
        exec.push_str(HIDDEN_ARG);
    }
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment=Telex email\nExec={exec}\n\
         Icon=telex\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        desktop_string(name)
    )
}

/// A program path for Exec: as it is when it has nothing special, else in double quotes with
/// `"`, `` ` ``, `$` and `\` escaped by a backslash; then backslashes doubled again, as every
/// string value of a desktop file escapes them.
fn exec_arg(path: &str) -> String {
    let plain = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c));
    let quoted = if plain {
        path.to_owned()
    } else {
        let mut q = String::from("\"");
        for c in path.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                q.push('\\');
            }
            q.push(c);
        }
        q.push('"');
        q
    };
    quoted.replace('\\', "\\\\").replace('%', "%%")
}

/// A desktop file's string value: one line, backslashes escaped.
fn desktop_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace(['\n', '\r'], " ")
}

/// Where Telex is set to start with the system, as asked for: `None` takes it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    /// [`entry_name`].
    pub name: String,
    /// The app's identifier: the autostart file's name on Linux.
    pub identifier: String,
    /// The program to start: the AppImage on Linux, when it runs as one.
    pub exe: PathBuf,
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APPROVED_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";

impl Autostart {
    /// Start with the system (`Some(hidden)`) or not (`None`). On Windows a value in this
    /// user's Run key (no administrator needed, and Task Manager's Startup apps list it); on
    /// Linux a file in ~/.config/autostart. Nothing else is written.
    pub fn set(&self, start: Option<bool>) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            match start {
                Some(hidden) => {
                    win::set_value(RUN_KEY, &self.name, &run_command(&self.exe, hidden))
                }
                None => win::delete_value(RUN_KEY, &self.name),
            }
        }
        #[cfg(not(windows))]
        {
            let Some(config) = config_dir() else {
                return Err(std::io::Error::other("no home folder"));
            };
            self.set_in(&config, start)
        }
    }

    /// The autostart file in `config` (~/.config): written, or taken away.
    #[cfg(not(windows))]
    fn set_in(&self, config: &Path, start: Option<bool>) -> std::io::Result<()> {
        let _ = RUN_KEY;
        let dir = config.join("autostart");
        let file = dir.join(format!("{}.desktop", self.identifier));
        match start {
            Some(hidden) => {
                std::fs::create_dir_all(&dir)?;
                std::fs::write(&file, desktop_entry(&self.name, &self.exe, hidden))
            }
            None => match std::fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        }
    }

    /// True when the user turned Telex off in Task Manager's Startup apps (or Settings › Apps
    /// › Startup): Windows then skips the Run value. Telex says so and leaves it to them.
    pub fn blocked(&self) -> bool {
        #[cfg(windows)]
        {
            // The value's first byte is even while it may start (2, 6) and odd once turned
            // off (3, 7), followed by when that was.
            win::binary_value(APPROVED_KEY, &self.name)
                .and_then(|v| v.first().copied())
                .is_some_and(|b| b & 1 == 1)
        }
        #[cfg(not(windows))]
        {
            let _ = APPROVED_KEY;
            false
        }
    }
}

/// The user's config folder on Linux: $XDG_CONFIG_HOME, else ~/.config.
#[cfg(not(windows))]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
        RRF_RT_REG_BINARY,
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn status(code: u32) -> std::io::Result<()> {
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(code as i32))
        }
    }

    pub fn set_value(key: &str, name: &str, data: &str) -> std::io::Result<()> {
        let (key, name, data) = (wide(key), wide(name), wide(data));
        let bytes = u32::try_from(data.len() * 2).map_err(std::io::Error::other)?;
        // SAFETY: every string is NUL-terminated, and `bytes` is the size of `data` with its NUL.
        status(unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                bytes,
            )
        })
    }

    pub fn delete_value(key: &str, name: &str) -> std::io::Result<()> {
        let (key, name) = (wide(key), wide(name));
        // SAFETY: both strings are NUL-terminated.
        let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        status(code)
    }

    pub fn binary_value(key: &str, name: &str) -> Option<Vec<u8>> {
        let (key, name) = (wide(key), wide(name));
        let mut buf = vec![0u8; 64];
        let mut size = buf.len() as u32;
        // SAFETY: both names are NUL-terminated; Windows writes at most `size` bytes into `buf`.
        let code = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_BINARY,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if code != ERROR_SUCCESS {
            return None;
        }
        buf.truncate(size as usize);
        Some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 18)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn u(email: &str, unread: usize) -> InboxUnread {
        InboxUnread {
            email: email.into(),
            unread,
        }
    }

    #[test]
    fn the_tray_counts_each_inbox() {
        let store = crate::sample::sample_store();
        let tree = store.folder_tree().unwrap();
        let unread = inbox_unread(&tree);
        assert!(!unread.is_empty());
        for (row, account) in unread.iter().zip(tree.accounts.iter()) {
            assert_eq!(row.email, account.email);
            let inbox = account.folder_of_kind(FolderKind::Inbox).unwrap();
            let want = tree
                .counts
                .get(&account.folder_key(&inbox.id))
                .map_or(0, |c| c.unread);
            assert_eq!(row.unread, want);
        }
        assert!(unread.iter().any(|u| u.unread > 0));
    }

    #[test]
    fn the_tooltip_names_the_accounts_that_have_mail() {
        i18n::with(Lang::En, || {
            assert_eq!(tooltip(&[]), "Telex");
            assert_eq!(tooltip(&[u("a@x.pl", 0)]), "Telex");
            assert_eq!(
                tooltip(&[u("a@x.pl", 1), u("b@x.pl", 0)]),
                "Telex · 1 unread message"
            );
            assert_eq!(
                tooltip(&[u("a@x.pl", 3), u("b@x.pl", 0), u("c@x.pl", 2)]),
                "Telex · 5 unread messages\na@x.pl: 3\nc@x.pl: 2"
            );
        });
        i18n::with(Lang::Pl, || {
            assert_eq!(unread_line(1), "1 nieprzeczytana wiadomość");
            assert_eq!(unread_line(3), "3 nieprzeczytane wiadomości");
            assert_eq!(unread_line(12), "12 nieprzeczytanych wiadomości");
        });
    }

    #[test]
    fn a_long_tooltip_stops_where_windows_cuts_it() {
        let many: Vec<InboxUnread> = (0..20)
            .map(|i| u(&format!("someone.long.name{i}@example-company.pl"), i + 1))
            .collect();
        let text = i18n::with(Lang::En, || tooltip(&many));
        assert!(text.encode_utf16().count() <= TOOLTIP_MOST, "{text}");
        assert!(text.ends_with("\n…"));
        assert!(text.contains("someone.long.name0@"));
        // One very long address alone is cut too.
        let one = [u(&"x".repeat(300), 2), u("b@x.pl", 1)];
        let text = i18n::with(Lang::En, || tooltip(&one));
        assert!(text.encode_utf16().count() <= TOOLTIP_MOST);
    }

    #[test]
    fn pauses_end_in_an_hour_or_in_the_morning() {
        assert_eq!(pause_until(Pause::Hour, at(14, 30)), Some(at(15, 30)));
        assert_eq!(
            pause_until(Pause::Morning, at(14, 30)),
            Some(at(8, 0) + Duration::days(1))
        );
        // Before eight, the morning is today's.
        assert_eq!(pause_until(Pause::Morning, at(1, 15)), Some(at(8, 0)));
        assert_eq!(
            pause_until(Pause::Morning, at(8, 0)),
            Some(at(8, 0) + Duration::days(1))
        );
        assert_eq!(pause_until(Pause::Off, at(9, 0)), None);
    }

    fn alpha(rgba: &[u8], size: usize, x: usize, y: usize) -> u8 {
        rgba[(y * size + x) * 4 + 3]
    }

    fn white(rgba: &[u8], size: usize) -> usize {
        rgba.chunks(4)
            .take(size * size)
            .filter(|p| p == &[255, 255, 255, 255])
            .count()
    }

    #[test]
    fn the_badge_is_a_red_shape_with_white_figures() {
        assert_eq!(badge_text(7), "7");
        assert_eq!(badge_text(42), "42");
        assert_eq!(badge_text(100), "99+");
        for size in [16usize, 20, 24, 32] {
            for count in [1, 8, 42, 100, 1234] {
                let img = badge(count, size as u32);
                assert_eq!(img.len(), size * size * 4);
                // Opaque in the middle, the figures white and a few.
                assert_eq!(alpha(&img, size, size / 2, 1.max(size / 8)), 255);
                assert!(white(&img, size) > 4, "{count} at {size}");
            }
        }
        // One figure: a circle, so its corners are clear; more: a square with round corners.
        let one = badge(3, 16);
        assert_eq!(alpha(&one, 16, 0, 0), 0);
        assert_eq!(alpha(&one, 16, 15, 15), 0);
        let two = badge(42, 16);
        assert_eq!(alpha(&two, 16, 0, 8), 255);
        // At 16 px one figure is drawn twice as large as the 3 × 5 font: 6 × 10.
        let lit: Vec<(usize, usize)> = (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .filter(|&(x, y)| one[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4] == [255; 4])
            .collect();
        let width =
            lit.iter().map(|p| p.0).max().unwrap() - lit.iter().map(|p| p.0).min().unwrap() + 1;
        let height =
            lit.iter().map(|p| p.1).max().unwrap() - lit.iter().map(|p| p.1).min().unwrap() + 1;
        assert_eq!((width, height), (6, 10));
    }

    #[test]
    fn autostart_entries_start_the_program_hidden_on_request() {
        assert_eq!(entry_name("app.telexmail.desktop"), "Telex");
        assert_eq!(
            entry_name("app.telexmail.devcheck"),
            "Telex (app.telexmail.devcheck)"
        );
        let exe = Path::new("C:\\Program Files\\Telex\\telex.exe");
        assert_eq!(
            run_command(exe, true),
            "\"C:\\Program Files\\Telex\\telex.exe\" --tray"
        );
        assert_eq!(
            run_command(exe, false),
            "\"C:\\Program Files\\Telex\\telex.exe\""
        );
        let file = desktop_entry("Telex", Path::new("/home/anna/Apps/Telex.AppImage"), true);
        assert!(file.starts_with("[Desktop Entry]\nType=Application\n"));
        assert!(file.contains("\nExec=/home/anna/Apps/Telex.AppImage --tray\n"));
        assert!(file.contains("\nName=Telex\n"));
        // Spaces and specials: quoted, escaped, and backslashes doubled for the file.
        let file = desktop_entry(
            "Telex",
            Path::new("/home/anna/My Apps/T$lex\\1%.AppImage"),
            false,
        );
        assert!(
            file.contains("\nExec=\"/home/anna/My Apps/T\\\\$lex\\\\\\\\1%%.AppImage\"\n"),
            "{file}"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn the_autostart_file_goes_into_the_config_folder() {
        let dir = std::env::temp_dir().join(format!("telex-autostart-{}", std::process::id()));
        let entry = Autostart {
            name: "Telex".into(),
            identifier: "app.telexmail.test".into(),
            exe: "/usr/bin/telex".into(),
        };
        entry.set_in(&dir, Some(true)).unwrap();
        let file = dir.join("autostart/app.telexmail.test.desktop");
        assert!(std::fs::read_to_string(&file)
            .unwrap()
            .contains("Exec=/usr/bin/telex --tray"));
        entry.set_in(&dir, None).unwrap();
        assert!(!file.exists());
        entry.set_in(&dir, None).unwrap();
        assert!(!entry.blocked());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
