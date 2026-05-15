//! Win32 window embedding — reparents an emulator's top-level window into
//! the Abyss main window so the user experiences "one app" instead of a
//! pop-out emulator window.
//!
//! Windows-only. The launcher checks `cfg(target_os)` before calling in.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use std::sync::Mutex;

use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumChildWindows, EnumWindows, GetClientRect, GetWindow,
    GetWindowLongPtrW, GetWindowThreadProcessId, IsWindowVisible, MoveWindow, SetParent,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE, GW_OWNER, HWND_TOP,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, SW_SHOWMAXIMIZED, WS_CAPTION, WS_CHILD,
    WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_THICKFRAME, WS_VISIBLE,
};

/// Children of the Abyss main HWND that we hid in [`reparent`] before
/// embedding an emulator. Stored as raw isize so the `!Send` `HWND`
/// doesn't poison an async future. Restored via [`restore_host_chrome`]
/// when the emulator exits.
static HIDDEN_HOST_CHILDREN: Mutex<Vec<isize>> = Mutex::new(Vec::new());

/// Wait up to `timeout` for a top-level window owned by `pid` to appear,
/// then reparent it into `host_hwnd_raw`, strip its OS chrome, and resize
/// to fill the host's client area.
pub async fn embed_window(host_hwnd_raw: isize, pid: u32, timeout: Duration) -> Result<()> {
    let target_raw = wait_for_window(pid, timeout).await
        .ok_or_else(|| anyhow!("no top-level window for pid {pid} within {timeout:?}"))?;
    tokio::task::spawn_blocking(move || reparent(target_raw, host_hwnd_raw))
        .await
        .context("reparent task panicked")??;
    Ok(())
}

/// Polls for a visible, ownerless top-level window owned by `pid`. The
/// returned `isize` is the raw `HWND` pointer — small enough to send
/// across thread/task boundaries without lugging the `!Send` `HWND` type
/// through async state.
async fn wait_for_window(pid: u32, timeout: Duration) -> Option<isize> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        let found = tokio::task::spawn_blocking(move || find_top_level_by_pid(pid))
            .await
            .ok()
            .flatten();
        if found.is_some() { return found; }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    None
}

struct Hunt {
    target_pid: u32,
    found:      Option<isize>,
}

fn find_top_level_by_pid(pid: u32) -> Option<isize> {
    let mut hunt = Hunt { target_pid: pid, found: None };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut hunt as *mut _ as isize));
    }
    hunt.found
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let hunt: &mut Hunt = &mut *(lparam.0 as *mut Hunt);
        let mut wpid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut wpid));
        if wpid != hunt.target_pid {
            return BOOL(1);
        }
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        // Top-level only — skip owned/child windows.
        if GetWindow(hwnd, GW_OWNER).is_ok() {
            return BOOL(1);
        }
        hunt.found = Some(hwnd.0 as isize);
        BOOL(0)
    }
}

struct ChildCollector {
    out: Vec<isize>,
    skip: isize,
}

unsafe extern "system" fn collect_children(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let coll: &mut ChildCollector = &mut *(lparam.0 as *mut ChildCollector);
        let raw = hwnd.0 as isize;
        if raw != coll.skip {
            coll.out.push(raw);
        }
        BOOL(1)
    }
}

/// Restore the host's previously-hidden children (WebView2, etc.). Idempotent.
/// Called by the launcher's exit watcher when an embedded emulator exits.
pub fn restore_host_chrome() {
    let drained: Vec<isize> = {
        let mut g = HIDDEN_HOST_CHILDREN.lock().expect("hidden-children lock poisoned");
        std::mem::take(&mut *g)
    };
    for raw in drained {
        let h = HWND(raw as *mut c_void);
        unsafe { let _ = ShowWindow(h, SW_SHOW); }
    }
}

fn reparent(target_raw: isize, host_raw: isize) -> Result<()> {
    let target = HWND(target_raw as *mut c_void);
    let host   = HWND(host_raw   as *mut c_void);
    unsafe {
        // Strip overlapping/popup chrome, add WS_CHILD.
        let style = GetWindowLongPtrW(target, GWL_STYLE);
        let stripped_mask: isize =
              WS_OVERLAPPEDWINDOW.0 as isize
            | WS_POPUP.0           as isize
            | WS_CAPTION.0         as isize
            | WS_THICKFRAME.0      as isize;
        let new_style = (style & !stripped_mask) | WS_CHILD.0 as isize | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(target, GWL_STYLE, new_style);

        let ex = GetWindowLongPtrW(target, GWL_EXSTYLE);
        SetWindowLongPtrW(target, GWL_EXSTYLE, ex & !(WS_EX_APPWINDOW.0 as isize));

        SetParent(target, Some(host)).context("SetParent")?;

        // Resize to fit host's client area.
        let mut rect = RECT::default();
        GetClientRect(host, &mut rect).context("GetClientRect")?;
        MoveWindow(target, 0, 0, rect.right - rect.left, rect.bottom - rect.top, true)
            .context("MoveWindow")?;
        let _ = ShowWindow(target, SW_SHOWMAXIMIZED);

        // The Tauri WebView2 child renders via DirectComposition and
        // composites visually *above* normal child windows regardless of
        // GDI Z-order — so the only reliable way to make the emulator
        // visible is to hide the WebView2 child while the game runs.
        // Collect every sibling under `host` (skipping our freshly-
        // reparented target), stash them for [`restore_host_chrome`],
        // and ShowWindow(SW_HIDE) each one.
        let mut coll = ChildCollector { out: Vec::new(), skip: target_raw };
        let _ = EnumChildWindows(
            Some(host),
            Some(collect_children),
            LPARAM(&mut coll as *mut _ as isize),
        );
        // Direct children of host only — EnumChildWindows recurses, but
        // grandchildren are hidden implicitly when their parent hides.
        // Dedupe to top-level children (parent == host) using GetWindow
        // would help; the simpler stash-everything works because Show
        // on already-visible windows is a no-op.
        {
            let mut g = HIDDEN_HOST_CHILDREN.lock().expect("hidden-children lock poisoned");
            *g = coll.out.clone();
        }
        for raw in coll.out {
            let h = HWND(raw as *mut c_void);
            let _ = ShowWindow(h, SW_HIDE);
        }

        // Z-order kick + focus so input goes to the game, not the host.
        let _ = SetWindowPos(
            target,
            Some(HWND_TOP),
            0, 0, 0, 0,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = BringWindowToTop(target);
        let _ = SetFocus(Some(target));
    }
    Ok(())
}
