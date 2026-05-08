use floem::peniko::kurbo::Size;

#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::{
            HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, GetWindowTextLengthW,
                GetWindowTextW, GetWindowThreadProcessId, MINMAXINFO, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_GETMINMAXINFO, WM_NCDESTROY,
            },
        },
    },
    core::BOOL,
};

#[cfg(windows)]
const MINIMUM_SIZE_SUBCLASS_ID: usize = 0x4c49_4d53;

#[cfg(windows)]
struct MinimumContentSize {
    width: f64,
    height: f64,
}

#[cfg(windows)]
struct WindowSearchState<'a> {
    title: &'a str,
    process_id: u32,
    hwnd: Option<HWND>,
}

pub fn set_minimum_content_size(window_title: &str, minimum_size: Size) {
    set_minimum_content_size_impl(window_title, minimum_size);
}

#[cfg(windows)]
fn set_minimum_content_size_impl(window_title: &str, minimum_size: Size) {
    let Some(hwnd) = find_current_process_window(window_title) else {
        return;
    };

    let data = Box::new(MinimumContentSize {
        width: minimum_size.width,
        height: minimum_size.height,
    });
    let data_ptr = Box::into_raw(data);
    let installed = unsafe {
        SetWindowSubclass(
            hwnd,
            Some(minimum_size_subclass_proc),
            MINIMUM_SIZE_SUBCLASS_ID,
            data_ptr as usize,
        )
    } == true;
    if !installed {
        unsafe {
            drop(Box::from_raw(data_ptr));
        }
    }
}

#[cfg(not(windows))]
fn set_minimum_content_size_impl(_window_title: &str, _minimum_size: Size) {}

#[cfg(windows)]
fn find_current_process_window(window_title: &str) -> Option<HWND> {
    let mut state = WindowSearchState {
        title: window_title,
        process_id: std::process::id(),
        hwnd: None,
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_for_title),
            LPARAM((&mut state as *mut WindowSearchState<'_>) as isize),
        );
    }
    state.hwnd
}

#[cfg(windows)]
unsafe extern "system" fn enum_windows_for_title(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = unsafe { &mut *(lparam.0 as *mut WindowSearchState<'_>) };

    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == state.process_id && window_text(hwnd) == state.title {
        state.hwnd = Some(hwnd);
        return BOOL(0);
    }

    BOOL(1)
}

#[cfg(windows)]
fn window_text(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }

    let mut buffer = vec![0; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..copied as usize])
}

#[cfg(windows)]
unsafe extern "system" fn minimum_size_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    ref_data: usize,
) -> LRESULT {
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };

    if message == WM_GETMINMAXINFO {
        let info = lparam.0 as *mut MINMAXINFO;
        let minimum_size = unsafe { (ref_data as *const MinimumContentSize).as_ref() };
        if let (Some(info), Some(minimum_size)) = (unsafe { info.as_mut() }, minimum_size) {
            let (width, height) = minimum_window_track_size(hwnd, minimum_size);
            info.ptMinTrackSize.x = info.ptMinTrackSize.x.max(width);
            info.ptMinTrackSize.y = info.ptMinTrackSize.y.max(height);
        }
    }

    if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(
                hwnd,
                Some(minimum_size_subclass_proc),
                MINIMUM_SIZE_SUBCLASS_ID,
            );
            drop(Box::from_raw(ref_data as *mut MinimumContentSize));
        }
    }

    result
}

#[cfg(windows)]
fn minimum_window_track_size(hwnd: HWND, minimum_size: &MinimumContentSize) -> (i32, i32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let scale = dpi as f64 / 96.0;
    let content_width = (minimum_size.width * scale).ceil() as i32;
    let content_height = (minimum_size.height * scale).ceil() as i32;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: content_width,
        bottom: content_height,
    };

    let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
    let ex_style = WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32);
    if unsafe { AdjustWindowRectExForDpi(&mut rect, style, false, ex_style, dpi) }.is_ok() {
        (
            (rect.right - rect.left).max(content_width),
            (rect.bottom - rect.top).max(content_height),
        )
    } else {
        (content_width, content_height)
    }
}
