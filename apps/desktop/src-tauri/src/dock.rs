//! The production host's move-end/work-area behavior, adapted to a Tauri window.
use std::{cell::Cell, io, sync::Mutex, time::Instant};
use tauri::{Emitter, Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::{
        CreateEllipticRgn, DeleteObject, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromWindow, SetWindowRgn,
    },
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const HOOK: usize = 0x4c4250;
const DOCK_STYLE: u32 = WS_CAPTION | WS_THICKFRAME | WS_MAXIMIZEBOX;
#[derive(Clone, Copy, Debug, PartialEq)]
enum Edge {
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
impl Bounds {
    fn array(self) -> [i32; 4] {
        [self.x, self.y, self.w, self.h]
    }
    fn fit(self, work: Self) -> Self {
        let w = self.w.min(work.w);
        let h = self.h.min(work.h);
        Self {
            x: self.x.clamp(work.x, work.x + work.w - w),
            y: self.y.clamp(work.y, work.y + work.h - h),
            w,
            h,
        }
    }
}
#[derive(Default)]
pub struct Dock {
    expanded: Option<Bounds>,
    was_topmost: bool,
    dock_style: u32,
}
struct Hook {
    window: WebviewWindow,
    moved: Cell<bool>,
    move_started: Cell<Option<Instant>>,
}

fn hwnd(window: &WebviewWindow) -> io::Result<HWND> {
    window
        .hwnd()
        .map(|handle| handle.0 as _)
        .map_err(io::Error::other)
}
fn geometry(window: &WebviewWindow) -> io::Result<(Bounds, Bounds, u32)> {
    let handle = hwnd(window)?;
    let mut rect = RECT::default();
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if GetWindowRect(handle, &mut rect) == 0
            || GetMonitorInfoW(
                MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST),
                &mut info,
            ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    let bounds = Bounds {
        x: rect.left,
        y: rect.top,
        w: rect.right - rect.left,
        h: rect.bottom - rect.top,
    };
    let rect = info.rcWork;
    Ok((
        bounds,
        Bounds {
            x: rect.left,
            y: rect.top,
            w: rect.right - rect.left,
            h: rect.bottom - rect.top,
        },
        unsafe { GetDpiForWindow(handle) }.max(96),
    ))
}
fn edge(bounds: Bounds, work: Bounds, dpi: u32) -> Option<Edge> {
    let padding = (12. * dpi as f64 / 96.).round() as i32;
    if bounds.x <= work.x + padding {
        Some(Edge::Left)
    } else if bounds.x + bounds.w >= work.x + work.w - padding {
        Some(Edge::Right)
    } else {
        None
    }
}
fn circle(bounds: Bounds, work: Bounds, dpi: u32, edge: Edge) -> Bounds {
    let size = ((40. * dpi as f64 / 96.).round() as i32)
        .min(work.w)
        .min(work.h);
    Bounds {
        x: if edge == Edge::Left {
            work.x
        } else {
            work.x + work.w - size
        },
        y: (bounds.y + (bounds.h - size) / 2).clamp(work.y, work.y + work.h - size),
        w: size,
        h: size,
    }
}
fn relocated(bounds: Bounds, expanded: Bounds, work: Bounds, dpi: u32) -> (Bounds, Bounds) {
    let edge = if bounds.x + bounds.w / 2 <= work.x + work.w / 2 {
        Edge::Left
    } else {
        Edge::Right
    };
    let docked = circle(bounds, work, dpi, edge);
    let expanded = Bounds {
        x: if edge == Edge::Left {
            work.x
        } else {
            work.x + work.w - expanded.w
        },
        y: docked.y + (docked.h - expanded.h) / 2,
        ..expanded
    }
    .fit(work);
    (docked, expanded)
}
fn redock(window: &WebviewWindow) -> io::Result<()> {
    let Some(expanded) = window.state::<Mutex<Dock>>().lock().unwrap().expanded else {
        return Ok(());
    };
    let (bounds, work, dpi) = geometry(window)?;
    let (docked, expanded) = relocated(bounds, expanded, work, dpi);
    position(window, docked, true)?;
    window.state::<Mutex<Dock>>().lock().unwrap().expanded = Some(expanded);
    shape(window)
}
fn position(window: &WebviewWindow, bounds: Bounds, topmost: bool) -> io::Result<()> {
    if unsafe {
        SetWindowPos(
            hwnd(window)?,
            if topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn set_dock_style(window: &WebviewWindow, flags: u32) -> io::Result<()> {
    let handle = hwnd(window)?;
    // Tao's cached visibility differs from our no-focus Win32 show/hide path.
    unsafe {
        let style = GetWindowLongPtrW(handle, GWL_STYLE) as u32;
        SetLastError(0);
        if SetWindowLongPtrW(handle, GWL_STYLE, ((style & !DOCK_STYLE) | flags) as isize) == 0
            && GetLastError() != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
pub fn is_docked(window: &WebviewWindow) -> bool {
    window
        .state::<Mutex<Dock>>()
        .lock()
        .unwrap()
        .expanded
        .is_some()
}
pub fn shape(window: &WebviewWindow) -> io::Result<()> {
    let handle = hwnd(window)?;
    if !is_docked(window) {
        unsafe {
            SetWindowRgn(handle, std::ptr::null_mut(), 1);
        }
        return Ok(());
    }
    let (bounds, _, _) = geometry(window)?;
    let region = unsafe { CreateEllipticRgn(0, 0, bounds.w, bounds.h) };
    if region.is_null() {
        return Err(io::Error::last_os_error());
    }
    if unsafe { SetWindowRgn(handle, region, 1) } == 0 {
        unsafe {
            DeleteObject(region);
        }
        return Err(io::Error::last_os_error());
    }
    // SetWindowRgn transfers ownership of a successfully applied region to Windows.
    Ok(())
}
pub fn collapse(window: &WebviewWindow) -> io::Result<bool> {
    let began = Instant::now();
    trace(window, "collapse_begin", serde_json::json!({}));
    let result = collapse_inner(window);
    trace(
        window,
        "collapse_end",
        serde_json::json!({"ok":result.is_ok(),"changed":result.as_ref().ok(),"elapsed_ms":began.elapsed().as_millis(),"os_error":result.as_ref().err().and_then(io::Error::raw_os_error)}),
    );
    result
}
fn collapse_inner(window: &WebviewWindow) -> io::Result<bool> {
    if is_docked(window) {
        return Ok(false);
    }
    let (bounds, work, dpi) = geometry(window)?;
    let Some(edge) = edge(bounds, work, dpi) else {
        return Ok(false);
    };
    let style = unsafe { GetWindowLongPtrW(hwnd(window)?, GWL_STYLE) } as u32;
    {
        let state = window.state::<Mutex<Dock>>();
        let mut state = state.lock().unwrap();
        state.expanded = Some(bounds.fit(work));
        state.was_topmost =
            unsafe { GetWindowLongPtrW(hwnd(window)?, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST != 0;
        state.dock_style = style & DOCK_STYLE;
    }
    super::visibility(window, false);
    let _ = window.emit("shell-docked", true);
    // Remove the native border as well: move/redock can repaint it despite WM_NCPAINT suppression.
    let result = set_dock_style(window, 0)
        .and_then(|_| position(window, circle(bounds, work, dpi, edge), true))
        .and_then(|_| shape(window));
    if let Err(error) = result {
        let _ = expand(window);
        return Err(error);
    }
    Ok(true)
}
pub fn expand(window: &WebviewWindow) -> io::Result<()> {
    if !is_docked(window) {
        return Ok(());
    }
    let began = Instant::now();
    trace(window, "expand_begin", serde_json::json!({}));
    let result = expand_inner(window);
    trace(
        window,
        "expand_end",
        serde_json::json!({"ok":result.is_ok(),"elapsed_ms":began.elapsed().as_millis(),"os_error":result.as_ref().err().and_then(io::Error::raw_os_error)}),
    );
    result
}
fn expand_inner(window: &WebviewWindow) -> io::Result<()> {
    let Some(saved) = window.state::<Mutex<Dock>>().lock().unwrap().expanded else {
        return Ok(());
    };
    let (_, work, _) = geometry(window)?;
    let (topmost, dock_style) = {
        let state = window.state::<Mutex<Dock>>();
        let state = state.lock().unwrap();
        (state.was_topmost, state.dock_style)
    };
    set_dock_style(window, dock_style)?;
    // State is released before resizing: native resize callbacks consult it synchronously.
    window.state::<Mutex<Dock>>().lock().unwrap().expanded = None;
    if let Err(error) = position(window, saved.fit(work), topmost) {
        window.state::<Mutex<Dock>>().lock().unwrap().expanded = Some(saved);
        return Err(error);
    }
    shape(window)?;
    let _ = window.emit("shell-docked", false);
    super::visibility(window, true);
    Ok(())
}
pub fn snapshot(window: &WebviewWindow) -> serde_json::Value {
    geometry(window).map(|(bounds,work,dpi)|{
        let handle=hwnd(window).ok();
        let style=handle.map(|h| unsafe { GetWindowLongPtrW(h,GWL_STYLE) } as u32).unwrap_or_default();
        serde_json::json!({"docked":is_docked(window),"bounds":bounds.array(),"work_area":work.array(),"dpi":dpi,
            "native_visible":handle.is_some_and(|h|unsafe {IsWindowVisible(h)}!=0),"minimized":handle.is_some_and(|h|unsafe {IsIconic(h)}!=0),
            "resizable":style&WS_THICKFRAME!=0,"maximizable":style&WS_MAXIMIZEBOX!=0})
    }).unwrap_or(serde_json::Value::Null)
}
#[track_caller]
pub fn trace(window: &WebviewWindow, event: &str, details: serde_json::Value) {
    lazybuddy_host_control::logs::event(
        "host",
        event,
        serde_json::json!({"window":snapshot(window),"details":details}),
    );
}
pub fn attach(window: &WebviewWindow) -> io::Result<()> {
    let handle = hwnd(window)?;
    let hook = Box::into_raw(Box::new(Hook {
        window: window.clone(),
        moved: Cell::new(false),
        move_started: Cell::new(None),
    }));
    if unsafe { SetWindowSubclass(handle, Some(subclass), HOOK, hook as usize) } == 0 {
        unsafe {
            drop(Box::from_raw(hook));
        }
        return Err(io::Error::last_os_error());
    }
    trace(window, "window_attached", serde_json::json!({}));
    Ok(())
}
unsafe extern "system" fn subclass(
    window: HWND,
    event: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    let hook = unsafe { &*(data as *const Hook) };
    match event {
        WM_NCACTIVATE if is_docked(&hook.window) => {
            // Keep Tao's focus events, but do not repaint system chrome over the alpha edge.
            return unsafe { DefSubclassProc(window, event, wparam, -1) };
        }
        WM_NCPAINT if is_docked(&hook.window) => return 0,
        WM_HOTKEY => {
            let visible =
                unsafe { IsWindowVisible(window) } != 0 && unsafe { IsIconic(window) } == 0;
            let action = if visible && !is_docked(&hook.window) {
                "hide"
            } else {
                "show"
            };
            let _ = crate::window_action(action, hook.window.clone());
        }
        WM_ENTERSIZEMOVE => {
            hook.moved.set(false);
            hook.move_started.set(Some(Instant::now()));
            trace(&hook.window, "move_begin", serde_json::json!({}));
        }
        WM_MOVING => hook.moved.set(true),
        WM_EXITSIZEMOVE => {
            hook.window
                .state::<crate::host::State>()
                .remember_window(&hook.window);
            let moved = hook.moved.replace(false);
            let mut error = None;
            if moved {
                let result = if is_docked(&hook.window) {
                    redock(&hook.window)
                } else {
                    collapse(&hook.window).map(|_| ())
                };
                if result.is_err() {
                    error = result.as_ref().err().and_then(io::Error::raw_os_error);
                    let _ = hook
                        .window
                        .emit("shell-error", "贴边停靠失败，请重新拖动窗口。");
                }
            }
            trace(
                &hook.window,
                "move_end",
                serde_json::json!({"moved":moved,"elapsed_ms":hook.move_started.take().map(|t|t.elapsed().as_millis()),"os_error":error}),
            );
        }
        WM_NCHITTEST if is_docked(&hook.window) => return HTCLIENT as LRESULT,
        WM_NCDESTROY => unsafe {
            RemoveWindowSubclass(window, Some(subclass), HOOK);
            drop(Box::from_raw(data as *mut Hook));
        },
        _ => {}
    }
    unsafe { DefSubclassProc(window, event, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_a_collapsed_window_snaps_and_restores_on_the_new_side() {
        let work = Bounds {
            x: -1920,
            y: 40,
            w: 1920,
            h: 1040,
        };
        let saved = Bounds {
            x: -1920,
            y: 200,
            w: 780,
            h: 540,
        };
        let (docked, expanded) = relocated(
            Bounds {
                x: -150,
                y: 300,
                w: 60,
                h: 60,
            },
            saved,
            work,
            144,
        );
        assert_eq!(
            docked,
            Bounds {
                x: -60,
                y: 300,
                w: 60,
                h: 60
            }
        );
        assert_eq!(
            expanded,
            Bounds {
                x: -780,
                y: 60,
                w: 780,
                h: 540
            }
        );
        let (docked, expanded) = relocated(
            Bounds {
                x: -1600,
                y: 10,
                w: 60,
                h: 60,
            },
            expanded,
            work,
            144,
        );
        assert_eq!(
            docked,
            Bounds {
                x: -1920,
                y: 40,
                w: 60,
                h: 60
            }
        );
        assert_eq!(
            expanded,
            Bounds {
                x: -1920,
                y: 40,
                w: 780,
                h: 540
            }
        );
    }
    #[test]
    fn moving_a_collapsed_window_fits_a_smaller_monitor_and_its_dpi() {
        let saved = Bounds {
            x: -1920,
            y: 200,
            w: 780,
            h: 540,
        };
        let work = Bounds {
            x: 0,
            y: 0,
            w: 640,
            h: 480,
        };
        let (docked, expanded) = relocated(
            Bounds {
                x: 550,
                y: 450,
                w: 60,
                h: 60,
            },
            saved,
            work,
            192,
        );
        assert_eq!(
            docked,
            Bounds {
                x: 560,
                y: 400,
                w: 80,
                h: 80
            }
        );
        assert_eq!(expanded, work);
    }
    #[test]
    fn docking_uses_work_area_and_only_near_left_or_right_edges() {
        let work = Bounds {
            x: -1920,
            y: 40,
            w: 1920,
            h: 1040,
        };
        assert_eq!(
            edge(
                Bounds {
                    x: -1910,
                    y: 80,
                    w: 780,
                    h: 540
                },
                work,
                96
            ),
            Some(Edge::Left)
        );
        assert_eq!(
            edge(
                Bounds {
                    x: -790,
                    y: 80,
                    w: 780,
                    h: 540
                },
                work,
                96
            ),
            Some(Edge::Right)
        );
        assert_eq!(
            edge(
                Bounds {
                    x: -1500,
                    y: 80,
                    w: 780,
                    h: 540
                },
                work,
                96
            ),
            None
        );
        assert_eq!(
            edge(
                Bounds {
                    x: -1902,
                    y: 80,
                    w: 780,
                    h: 540
                },
                work,
                144
            ),
            Some(Edge::Left)
        );
    }
    #[test]
    fn logo_is_40_logical_pixels_and_stays_in_work_area() {
        let work = Bounds {
            x: -1920,
            y: 40,
            w: 1920,
            h: 1000,
        };
        assert_eq!(
            circle(
                Bounds {
                    x: -1900,
                    y: -200,
                    w: 780,
                    h: 400
                },
                work,
                96,
                Edge::Left
            ),
            Bounds {
                x: -1920,
                y: 40,
                w: 40,
                h: 40
            }
        );
        assert_eq!(
            circle(
                Bounds {
                    x: -500,
                    y: 1000,
                    w: 780,
                    h: 540
                },
                work,
                144,
                Edge::Right
            ),
            Bounds {
                x: -60,
                y: 980,
                w: 60,
                h: 60
            }
        );
    }
    #[test]
    fn expanded_window_fits_after_monitor_or_work_area_changes() {
        assert_eq!(
            Bounds {
                x: -2200,
                y: -100,
                w: 780,
                h: 540
            }
            .fit(Bounds {
                x: 0,
                y: 30,
                w: 640,
                h: 480
            }),
            Bounds {
                x: 0,
                y: 30,
                w: 640,
                h: 480
            }
        );
    }
}
