//! The tray icon (Windows' notification area, an AppIndicator on Linux) and how the main
//! window lives on the desktop, as Settings › Window says: hidden in the tray by its close
//! button or by minimizing, shown again from the icon, its menu or a second start; the
//! taskbar button's unread badge; notifications paused for a while; starting with the system.
//! The words and pictures come from `telex_core::desktop`.
//!
//! Hiding isn't closing: the window and its script keep running, so checks, IMAP IDLE, send
//! later and notifications go on. Quit closes the window for real (an open message is saved
//! as a draft first, as when the close button quits), and the app ends with it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use chrono::{Local, NaiveDateTime};
use serde::Serialize;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Wry};
use telex_core::desktop::{self, Autostart, InboxUnread, Pause};
use telex_core::settings::{Settings, StartWith, Tray};
use telex_core::{tr, FolderTree};

use crate::settings::SettingsState;
use crate::{with_store, AppState, CmdResult};

const ICON: &[u8] = include_bytes!("../icons/tray.png");
const ICON_UNREAD: &[u8] = include_bytes!("../icons/tray-unread.png");

#[derive(Default)]
pub struct TrayState {
    icon: Mutex<Option<TrayIcon<Wry>>>,
    /// The icon couldn't be made (Linux without an AppIndicator library): nothing hides in
    /// the tray then, so the close button quits and minimizing stays on the taskbar.
    failed: AtomicBool,
    /// What the icon shows now.
    unread: Mutex<Vec<InboxUnread>>,
    /// Notifications paused: `Some(None)` until turned back on, `Some(Some(t))` until `t`.
    paused: Mutex<Option<Option<NaiveDateTime>>>,
    /// Quit was chosen: the window's close request closes it, whatever the settings say.
    quitting: AtomicBool,
    /// The count on the taskbar button now (`None`: no badge).
    badge: Mutex<Option<usize>>,
}

fn lock<T: Default>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A thread that panicked while holding one of these left plain values behind.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn settings(app: &AppHandle) -> Settings {
    app.state::<SettingsState>().get()
}

/// At start: the icon, the taskbar button, starting with the system, and the window shown
/// unless Telex started hidden (`--tray`, from starting with the system).
pub fn start(app: &AppHandle) {
    let hidden = std::env::args().any(|a| a == desktop::HIDDEN_ARG);
    let now = settings(app);
    let unread = with_store(&app.state::<AppState>(), |s| s.folder_tree())
        .map(|tree| desktop::inbox_unread(&tree))
        .unwrap_or_default();
    *lock(&app.state::<TrayState>().unread) = unread;
    apply(app);
    // Where the program is may have changed (an update, an AppImage moved): written again.
    let _ = autostart(app, now.start_with_system);
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    if hidden && has_icon(app) {
        return;
    }
    let _ = main.show();
    if hidden {
        let _ = main.minimize();
    } else {
        let _ = main.set_focus();
    }
    badge_soon(app);
}

/// The settings changed: the icon shown or taken away, the taskbar button, the badge.
pub fn apply(app: &AppHandle) {
    let s = settings(app);
    let state = app.state::<TrayState>();
    {
        let mut icon = lock(&state.icon);
        if s.tray == Tray::Off {
            if let Some(icon) = icon.as_ref() {
                let _ = icon.set_visible(false);
            }
        } else if let Some(icon) = icon.as_ref() {
            let _ = icon.set_visible(true);
        } else if !state.failed.load(Ordering::SeqCst) {
            match build(app) {
                Some(made) => *icon = Some(made),
                None => state.failed.store(true, Ordering::SeqCst),
            }
        }
    }
    let shown = has_icon(app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_skip_taskbar(shown && s.tray == Tray::Only);
        // Hidden in a tray that is gone: back on screen, or it couldn't be reached.
        if !shown && !main.is_visible().unwrap_or(true) {
            let _ = main.show();
        }
    }
    redraw(app);
    set_badge(app, true);
    // Again once a taskbar button that came back exists.
    badge_soon(app);
}

/// True when the tray icon is on screen, so the window may hide in it.
fn has_icon(app: &AppHandle) -> bool {
    settings(app).tray != Tray::Off && lock(&app.state::<TrayState>().icon).is_some()
}

fn picture(unread: bool) -> Option<Image<'static>> {
    let (rgba, width, height) = desktop::png_rgba(if unread { ICON_UNREAD } else { ICON })?;
    Some(Image::new_owned(rgba, width, height))
}

fn build(app: &AppHandle) -> Option<TrayIcon<Wry>> {
    let make = || {
        let mut builder = TrayIconBuilder::with_id("telex")
            .tooltip("Telex")
            .menu(&menu(app).ok()?)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    toggle(tray.app_handle());
                }
            });
        if let Some(icon) = picture(false) {
            builder = builder.icon(icon);
        }
        builder.build(app).ok()
    };
    // Linux loads the AppIndicator library when the first icon is made, and panics without
    // one; Telex then goes on without a tray.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(make))
        .ok()
        .flatten()
}

/// The tray's menu as things are now.
fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<TrayState>();
    let total: usize = lock(&state.unread).iter().map(|u| u.unread).sum();
    let open = MenuItem::with_id(
        app,
        "open",
        tr!("Open Telex", "Otwórz Telexa"),
        true,
        None::<&str>,
    )?;
    let check = MenuItem::with_id(
        app,
        "check",
        tr!("Get mail", "Odbierz pocztę"),
        true,
        None::<&str>,
    )?;
    let compose = MenuItem::with_id(
        app,
        "compose",
        tr!("New message", "Nowa wiadomość"),
        true,
        None::<&str>,
    )?;
    let unread = if total == 0 {
        MenuItem::with_id(
            app,
            "unread",
            tr!("No unread mail", "Brak nieprzeczytanej poczty"),
            false,
            None::<&str>,
        )?
    } else {
        MenuItem::with_id(
            app,
            "unread",
            desktop::unread_line(total),
            true,
            None::<&str>,
        )?
    };
    let notify = settings(app).notify;
    let pause = match paused_until(app) {
        Some(until) => {
            let line = match until {
                Some(at) => {
                    let time = at.format("%H:%M").to_string();
                    tr!(
                        "Notifications paused until {time}",
                        "Powiadomienia wstrzymane do {time}"
                    )
                }
                None => tr!("Notifications paused", "Powiadomienia wstrzymane"),
            };
            let line = MenuItem::with_id(app, "paused", line, false, None::<&str>)?;
            let resume = MenuItem::with_id(
                app,
                "resume",
                tr!("Resume notifications", "Wznów powiadomienia"),
                true,
                None::<&str>,
            )?;
            Submenu::with_id_and_items(
                app,
                "pause",
                tr!("Notifications", "Powiadomienia"),
                true,
                &[&line, &resume],
            )?
        }
        None => {
            let hour = MenuItem::with_id(
                app,
                "pause-hour",
                tr!("For an hour", "Na godzinę"),
                true,
                None::<&str>,
            )?;
            let morning = MenuItem::with_id(
                app,
                "pause-morning",
                tr!("Until the morning (08:00)", "Do rana (8:00)"),
                true,
                None::<&str>,
            )?;
            let off = MenuItem::with_id(
                app,
                "pause-off",
                tr!("Until I turn them back on", "Dopóki ich nie wznowię"),
                true,
                None::<&str>,
            )?;
            Submenu::with_id_and_items(
                app,
                "pause",
                tr!("Pause notifications", "Wstrzymaj powiadomienia"),
                notify,
                &[&hour, &morning, &off],
            )?
        }
    };
    let quit = MenuItem::with_id(
        app,
        "quit",
        tr!("Quit Telex", "Zamknij Telexa"),
        true,
        None::<&str>,
    )?;
    Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &check,
            &compose,
            &PredefinedMenuItem::separator(app)?,
            &unread,
            &pause,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

/// The icon, its tooltip and its menu drawn again from what is known now.
fn redraw(app: &AppHandle) {
    let state = app.state::<TrayState>();
    let unread = lock(&state.unread).clone();
    let icon = lock(&state.icon);
    let Some(icon) = icon.as_ref() else {
        return;
    };
    let any = unread.iter().any(|u| u.unread > 0);
    let _ = icon.set_icon(picture(any));
    let _ = icon.set_tooltip(Some(desktop::tooltip(&unread)));
    if let Ok(menu) = menu(app) {
        let _ = icon.set_menu(Some(menu));
    }
}

/// The folder tree was read again (after any change): the icon, tooltip, menu and badge
/// follow the unread mail in the Inboxes.
pub fn refresh(app: &AppHandle, tree: &FolderTree) {
    let unread = desktop::inbox_unread(tree);
    {
        let state = app.state::<TrayState>();
        let mut last = lock(&state.unread);
        if *last == unread {
            return;
        }
        *last = unread;
    }
    redraw(app);
    set_badge(app, false);
}

/// The unread count on the taskbar button: Windows' overlay icon, drawn at the size Windows
/// shows it; elsewhere the launcher's count (KDE, Ubuntu's dock). Not with the tray only,
/// where there is no button. `again`: set even when the count is the same (the button was
/// made anew after the window was hidden).
fn set_badge(app: &AppHandle, again: bool) {
    let s = settings(app);
    let state = app.state::<TrayState>();
    let total: usize = lock(&state.unread).iter().map(|u| u.unread).sum();
    let want =
        (s.taskbar_badge && !(s.tray == Tray::Only && has_icon(app)) && total > 0).then_some(total);
    {
        let mut shown = lock(&state.badge);
        if !again && *shown == want {
            return;
        }
        *shown = want;
    }
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    #[cfg(windows)]
    {
        let scale = main.scale_factor().unwrap_or(1.0);
        let size = (16.0 * scale).round().clamp(16.0, 64.0) as u32;
        let _ = main
            .set_overlay_icon(want.map(|n| Image::new_owned(desktop::badge(n, size), size, size)));
    }
    #[cfg(not(windows))]
    {
        let _ = main.set_badge_count(want.map(|n| n as i64));
    }
}

/// The badge again a moment after the window shows: Windows makes its taskbar button anew,
/// without the overlay.
fn badge_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        set_badge(&app, true);
    });
}

/// The window on screen and in front: from the tray, a second start, or the menu.
pub fn show_main(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let _ = main.show();
    let _ = main.unminimize();
    let _ = main.set_focus();
    badge_soon(app);
}

fn hide_main(main: &WebviewWindow) {
    let _ = main.hide();
}

/// A left click on the icon: the window hides when it is on screen, else it shows.
fn toggle(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let on_screen = main.is_visible().unwrap_or(false) && !main.is_minimized().unwrap_or(false);
    if on_screen {
        hide_main(&main);
    } else {
        show_main(app);
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    let tell = |what: &str| {
        let _ = app.emit_to("main", "tray", what);
    };
    match id {
        "open" => show_main(app),
        "check" => tell("check"),
        "compose" => {
            show_main(app);
            tell("compose");
        }
        "unread" => {
            show_main(app);
            tell("unread");
        }
        "pause-hour" => pause(app, Pause::Hour),
        "pause-morning" => pause(app, Pause::Morning),
        "pause-off" => pause(app, Pause::Off),
        "resume" => {
            *lock(&app.state::<TrayState>().paused) = None;
            redraw(app);
        }
        "quit" => quit(app),
        _ => {}
    }
}

fn pause(app: &AppHandle, how: Pause) {
    let until = desktop::pause_until(how, Local::now().naive_local());
    *lock(&app.state::<TrayState>().paused) = Some(until);
    redraw(app);
    // The menu says when they come back; once they have, it offers the pause again.
    if let Some(at) = until {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let left = (at - Local::now().naive_local())
                .to_std()
                .unwrap_or_default();
            tokio::time::sleep(left + Duration::from_secs(1)).await;
            if paused_until(&app).is_none() {
                redraw(&app);
            }
        });
    }
}

/// Until when notifications are paused, if they are: `Some(None)` until turned back on.
/// A pause that is over ends here.
fn paused_until(app: &AppHandle) -> Option<Option<NaiveDateTime>> {
    let state = app.state::<TrayState>();
    let mut paused = lock(&state.paused);
    if let Some(Some(at)) = *paused {
        if at <= Local::now().naive_local() {
            *paused = None;
        }
    }
    *paused
}

/// New-mail notifications wait while the tray's Pause notifications lasts.
pub fn notifications_paused(app: &AppHandle) -> bool {
    paused_until(app).is_some()
}

/// Quit for real: the window closes (an open message saved as a draft first), and Telex
/// with it. If the window's script doesn't answer within a while, Telex ends anyway.
fn quit(app: &AppHandle) {
    app.state::<TrayState>()
        .quitting
        .store(true, Ordering::SeqCst);
    let Some(main) = app.get_webview_window("main") else {
        app.exit(0);
        return;
    };
    let _ = main.close();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        app.exit(0);
    });
}

/// The main window was minimized: into the tray when the settings say so.
pub fn minimized(window: &tauri::Window) {
    if window.label() != "main" || !window.is_minimized().unwrap_or(false) {
        return;
    }
    let app = window.app_handle();
    if settings(app).minimizes_to_tray() && has_icon(app) {
        let _ = window.hide();
    }
}

/// Asked by the window when its close button (or Alt+F4) is pressed: true when it went into
/// the tray instead, so the window stays and keeps working; false when it closes (then it
/// saves an open message as a draft first).
#[tauri::command(async)]
pub fn close_requested(app: AppHandle, window: WebviewWindow) -> bool {
    if window.label() != "main" || app.state::<TrayState>().quitting.load(Ordering::SeqCst) {
        return false;
    }
    if settings(&app).closes_to_tray() && has_icon(&app) {
        hide_main(&window);
        return true;
    }
    false
}

/// What Settings › Window needs to know beyond the settings.
#[derive(Debug, Serialize)]
pub struct DesktopState {
    /// The tray icon can be shown here (false: Linux without AppIndicator).
    tray: bool,
    /// Starting with the system is on here, but turned off in Task Manager's Startup apps.
    autostart_blocked: bool,
    /// Windows: the taskbar's own settings decide whether the icon shows outside the
    /// overflow, and the badge is an overlay icon.
    windows: bool,
}

#[tauri::command(async)]
pub fn desktop_state(app: AppHandle) -> DesktopState {
    let s = settings(&app);
    DesktopState {
        tray: !app.state::<TrayState>().failed.load(Ordering::SeqCst),
        autostart_blocked: s.start_with_system != StartWith::Off && entry(&app).blocked(),
        windows: cfg!(windows),
    }
}

/// Windows' taskbar settings, where the user turns on "Other system tray icons › Telex" to
/// keep its icon always visible. Telex never changes that itself.
#[tauri::command(async)]
pub fn open_taskbar_settings(app: AppHandle) -> CmdResult<()> {
    #[cfg(windows)]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url("ms-settings:taskbar", None::<&str>)
            .map_err(|e| {
                tr!(
                    "Couldn't open Windows' settings: {e}",
                    "Nie udało się otworzyć ustawień Windows: {e}"
                )
            })
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Ok(())
    }
}

/// How Telex starts with the system: its entry's name, this program, and on Linux the
/// AppImage when it runs as one (the program itself is inside it, in a folder that goes).
fn entry(app: &AppHandle) -> Autostart {
    let identifier = app.config().identifier.clone();
    let appimage = std::env::var_os("APPIMAGE")
        .filter(|_| cfg!(target_os = "linux"))
        .map(std::path::PathBuf::from);
    let exe = appimage
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_default();
    Autostart {
        name: desktop::entry_name(&identifier),
        identifier,
        exe,
    }
}

/// Start with the system as `start` says (Windows' Run key for this user, or an autostart
/// file on Linux), before the setting is saved.
pub fn autostart(app: &AppHandle, start: StartWith) -> CmdResult<()> {
    let hidden = match start {
        StartWith::Off => None,
        StartWith::Open => Some(false),
        StartWith::Tray => Some(true),
    };
    entry(app).set(hidden).map_err(|e| {
        tr!(
            "Couldn't change how Telex starts with the system: {e}",
            "Nie udało się zmienić uruchamiania Telexa z systemem: {e}"
        )
    })
}

/// The settings that change how the window lives on the desktop.
pub fn window_changed(before: &Settings, after: &Settings) -> bool {
    before.tray != after.tray
        || before.taskbar_badge != after.taskbar_badge
        || before.notify != after.notify
        || before.language != after.language
}
