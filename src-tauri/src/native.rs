//! What the system's own parts do for Telex where Tauri has no call for it: a page saved as a
//! PDF by WebView2 (Windows), and files dragged out of a window onto Explorer, the desktop or
//! a file manager (the shell's drag on Windows, GTK's on Linux). Both run on the window's
//! thread, as the system wants.

use std::path::PathBuf;

use tauri::WebviewWindow;
use telex_core::tr;

use crate::{failed, CmdResult};

/// Whether [`print_to_pdf`] writes a PDF here. Elsewhere the window opens the print dialog.
pub const CAN_PRINT_TO_PDF: bool = cfg!(windows);

/// The window's page, in its print layout, written to `path` as a PDF: A4 or Letter as the
/// system says, backgrounds kept, no header or footer.
#[cfg(windows)]
pub async fn print_to_pdf(window: &WebviewWindow, path: PathBuf) -> CmdResult<()> {
    use std::sync::{Arc, Mutex};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Environment6, ICoreWebView2_7,
    };
    use webview2_com::PrintToPdfCompletedHandler;
    use windows::core::{Interface, HSTRING};

    type Reply = tokio::sync::oneshot::Sender<Result<bool, String>>;
    fn answer(reply: &Mutex<Option<Reply>>, result: Result<bool, String>) {
        if let Some(tx) = reply.lock().ok().and_then(|mut r| r.take()) {
            let _ = tx.send(result);
        }
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    let reply = Arc::new(Mutex::new(Some(tx)));
    let later = Arc::clone(&reply);
    window
        .with_webview(move |webview| {
            let done = Arc::clone(&later);
            // SAFETY: WebView2's own COM calls, made on the window's thread (with_webview runs
            // there), on the webview Tauri hands over; the file name lives until the call returns.
            let started = unsafe {
                (|| -> windows::core::Result<()> {
                    let core: ICoreWebView2_7 = webview.controller().CoreWebView2()?.cast()?;
                    let env: ICoreWebView2Environment6 = webview.environment().cast()?;
                    let settings = env.CreatePrintSettings()?;
                    settings.SetShouldPrintBackgrounds(true)?;
                    settings.SetShouldPrintHeaderAndFooter(false)?;
                    // 15 mm (in inches), as @page in styles.css says for the print dialog.
                    settings.SetMarginTop(0.59)?;
                    settings.SetMarginBottom(0.59)?;
                    settings.SetMarginLeft(0.59)?;
                    settings.SetMarginRight(0.59)?;
                    let handler = PrintToPdfCompletedHandler::create(Box::new(move |hr, ok| {
                        answer(&done, hr.map(|()| ok).map_err(|e| e.message()));
                        Ok(())
                    }));
                    core.PrintToPdf(&HSTRING::from(path.as_os_str()), &settings, &handler)
                })()
            };
            if let Err(e) = started {
                answer(&later, Err(e.message()));
            }
        })
        .map_err(failed)?;
    match rx.await {
        Ok(Ok(true)) => Ok(()),
        Ok(Ok(false)) => Err(tr!(
            "WebView2 didn't write the file",
            "WebView2 nie zapisał pliku"
        )),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(failed(e)),
    }
}

#[cfg(not(windows))]
pub async fn print_to_pdf(_: &WebviewWindow, _: PathBuf) -> CmdResult<()> {
    Err(tr!(
        "Use the print dialog to save a PDF here.",
        "Tutaj zapisz PDF w oknie drukowania."
    ))
}

/// Drag these files out of the window, from where the pointer is, while its button is down:
/// the system's own drag, which Explorer, the desktop and other programs take as copies.
/// True when they were dropped somewhere, false when the drag was cancelled.
pub async fn drag_files(window: &WebviewWindow, paths: Vec<PathBuf>) -> CmdResult<bool> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let on = window.clone();
    window
        .run_on_main_thread(move || start_drag(&on, paths, tx))
        .map_err(failed)?;
    rx.await.map_err(failed)?
}

type Dropped = tokio::sync::oneshot::Sender<CmdResult<bool>>;

/// Windows: the shell makes the data object (as Explorer's own drag does: file names, the
/// files' icons in the drag image) and runs the drag with its default drop source, which
/// drops when the button goes up and cancels on Esc. It returns when the drag ends.
#[cfg(windows)]
fn start_drag(window: &WebviewWindow, paths: Vec<PathBuf>, done: Dropped) {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{IDropSource, DROPEFFECT_COPY, DROPEFFECT_NONE};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        BHID_DataObject, ILCreateFromPathW, ILFree, IShellItemArray,
        SHCreateShellItemArrayFromIDLists, SHDoDragDrop,
    };

    let result = (|| -> CmdResult<bool> {
        let hwnd = window.hwnd().map_err(failed)?;
        // SAFETY: shell calls on the window's thread, which has OLE set up (the webview's own
        // drop target needs it); every item list made here is freed below, after the drag.
        unsafe {
            let items: Vec<*mut ITEMIDLIST> = paths
                .iter()
                .map(|p| ILCreateFromPathW(&HSTRING::from(p.as_os_str())))
                .collect();
            let dragged = if items.iter().any(|i| i.is_null()) {
                Err(windows::core::Error::from_thread())
            } else {
                let list: Vec<*const ITEMIDLIST> = items.iter().map(|i| i.cast_const()).collect();
                SHCreateShellItemArrayFromIDLists(&list)
                    .and_then(|array: IShellItemArray| {
                        array.BindToHandler::<_, IDataObject>(None, &BHID_DataObject)
                    })
                    .and_then(|data| {
                        SHDoDragDrop(Some(hwnd), &data, None::<&IDropSource>, DROPEFFECT_COPY)
                    })
            };
            for item in items.into_iter().filter(|i| !i.is_null()) {
                ILFree(Some(item.cast_const()));
            }
            dragged
                .map(|effect| effect != DROPEFFECT_NONE)
                .map_err(|e| e.message())
        }
    })();
    let _ = done.send(result);
}

/// Linux: GTK's drag from the window, offering the files as file:// addresses (what file
/// managers take). It ends with "drag-end"; "drag-failed" before it means it was cancelled.
#[cfg(target_os = "linux")]
fn start_drag(window: &WebviewWindow, paths: Vec<PathBuf>, done: Dropped) {
    use gtk::gdk;
    use gtk::glib;
    use gtk::prelude::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    let window = match window.gtk_window() {
        Ok(w) => w,
        Err(e) => {
            let _ = done.send(Err(failed(e)));
            return;
        }
    };
    let uris: Vec<String> = paths
        .iter()
        .filter_map(|p| glib::filename_to_uri(p, None).ok())
        .map(|u| u.to_string())
        .collect();
    window.drag_source_set(gdk::ModifierType::BUTTON1_MASK, &[], gdk::DragAction::COPY);
    window.drag_source_add_uri_targets();
    let give = window.connect_drag_data_get(move |_, _, data, _, _| {
        let uris: Vec<&str> = uris.iter().map(String::as_str).collect();
        data.set_uris(&uris);
    });
    let cancelled = Rc::new(Cell::new(false));
    let seen = Rc::clone(&cancelled);
    let failed_id = window.connect_drag_failed(move |_, _, _| {
        seen.set(true);
        glib::Propagation::Proceed
    });
    let done = Rc::new(RefCell::new(Some(done)));
    let handlers = Rc::new(RefCell::new(vec![give, failed_id]));
    let (end_done, end_handlers) = (Rc::clone(&done), Rc::clone(&handlers));
    let end = window.connect_drag_end(move |w, _| {
        if let Some(tx) = end_done.borrow_mut().take() {
            let _ = tx.send(Ok(!cancelled.get()));
        }
        // The handlers go once this one has returned.
        let (w, ids) = (
            w.clone(),
            end_handlers.borrow_mut().drain(..).collect::<Vec<_>>(),
        );
        glib::idle_add_local_once(move || {
            for id in ids {
                w.disconnect(id);
            }
            w.drag_source_unset();
        });
    });
    handlers.borrow_mut().push(end);
    let started = window.drag_source_get_target_list().and_then(|targets| {
        window.drag_begin_with_coordinates(&targets, gdk::DragAction::COPY, 1, None, -1, -1)
    });
    if started.is_none() {
        for id in handlers.borrow_mut().drain(..) {
            window.disconnect(id);
        }
        window.drag_source_unset();
        if let Some(tx) = done.borrow_mut().take() {
            let _ = tx.send(Err(tr!(
                "The drag didn't start.",
                "Przeciąganie się nie zaczęło."
            )));
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn start_drag(_: &WebviewWindow, _: Vec<PathBuf>, done: Dropped) {
    let _ = done.send(Ok(false));
}
