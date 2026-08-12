use std::{ffi::c_void, io, ptr, sync::Mutex};

const TITLEBAR_HEIGHT_CSS: u32 = 42;
const CAPTION_BUTTON_WIDTH_CSS: u32 = 46;
const BASE_DPI: u64 = 96;
const SNAP_SUBCLASS_ID: usize = 0x4741_534E;

const CS_HREDRAW: u32 = 0x0002;
const CS_VREDRAW: u32 = 0x0001;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
const SWP_SHOWWINDOW: u32 = 0x0040;
const SWP_ASYNCWINDOWPOS: u32 = 0x4000;

const WM_CLOSE: u32 = 0x0010;
const WM_SIZE: u32 = 0x0005;
const WM_NCHITTEST: u32 = 0x0084;
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const WM_NCLBUTTONUP: u32 = 0x00A2;
const WM_SYSCOMMAND: u32 = 0x0112;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_SETICON: u32 = 0x0080;
const SC_MAXIMIZE: usize = 0xF030;
const SC_RESTORE: usize = 0xF120;
const HTMAXBUTTON: isize = 9;
const NULL_BRUSH: i32 = 5;
const ICON_BIG: usize = 1;
const IMAGE_ICON: u32 = 1;
const LR_DEFAULTSIZE: u32 = 0x0040;
const LR_SHARED: u32 = 0x8000;
const APP_ICON_RESOURCE_ID: usize = 32512;

const SNAP_CLASS: &[u16] = &[
    b'G' as u16,
    b'i' as u16,
    b't' as u16,
    b'A' as u16,
    b'c' as u16,
    b'o' as u16,
    b'r' as u16,
    b'n' as u16,
    b'S' as u16,
    b'n' as u16,
    b'a' as u16,
    b'p' as u16,
    b'O' as u16,
    b'v' as u16,
    b'e' as u16,
    b'r' as u16,
    b'l' as u16,
    b'a' as u16,
    b'y' as u16,
    0,
];

type Hwnd = *mut c_void;
type Hinstance = *mut c_void;
type Hbrush = *mut c_void;
type WndProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize;
type SubclassProc = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct WndClassEx {
    size: u32,
    style: u32,
    window_proc: Option<WndProc>,
    class_extra: i32,
    window_extra: i32,
    instance: Hinstance,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: Hbrush,
    menu_name: *const u16,
    class_name: *const u16,
    small_icon: *mut c_void,
}

#[derive(Clone, Copy)]
struct SnapState {
    parent: isize,
    overlay: isize,
}

static SNAP_STATE: Mutex<Option<SnapState>> = Mutex::new(None);

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(
        window: Hwnd,
        callback: Option<SubclassProc>,
        subclass_id: usize,
        reference_data: usize,
    ) -> i32;
    fn RemoveWindowSubclass(
        window: Hwnd,
        callback: Option<SubclassProc>,
        subclass_id: usize,
    ) -> i32;
    fn DefSubclassProc(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetStockObject(index: i32) -> *mut c_void;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(window_class: *const WndClassEx) -> u16;
    fn CreateWindowExW(
        extended_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: *mut c_void,
        instance: Hinstance,
        parameter: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(window: Hwnd) -> i32;
    fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn GetDpiForWindow(window: Hwnd) -> u32;
    fn GetParent(window: Hwnd) -> Hwnd;
    fn IsZoomed(window: Hwnd) -> i32;
    fn LoadImageW(
        instance: Hinstance,
        name: *const u16,
        image_type: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> *mut c_void;
    fn PostMessageW(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SendMessageW(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
    fn SetWindowPos(
        window: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
}

pub fn install(window: &tauri::WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    let parent = window.hwnd()?.0 as Hwnd;

    // SAFETY: Installation runs on Tauri's window thread with a live Win32 window handle.
    unsafe {
        install_app_icon(parent)?;
        register_overlay_class();
        remove_existing_overlay();

        let overlay = CreateWindowExW(
            0,
            SNAP_CLASS.as_ptr(),
            SNAP_CLASS.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            parent,
            ptr::null_mut(),
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );
        if overlay.is_null() {
            return Err(Box::new(io::Error::last_os_error()));
        }

        if SetWindowSubclass(parent, Some(parent_subclass_proc), SNAP_SUBCLASS_ID, 0) == 0 {
            DestroyWindow(overlay);
            return Err(Box::new(io::Error::last_os_error()));
        }

        *SNAP_STATE.lock().expect("snap state poisoned") = Some(SnapState {
            parent: parent as isize,
            overlay: overlay as isize,
        });
        update_overlay_position(parent);
    }

    Ok(())
}

unsafe fn install_app_icon(window: Hwnd) -> Result<(), Box<dyn std::error::Error>> {
    let icon = unsafe {
        LoadImageW(
            GetModuleHandleW(ptr::null()),
            APP_ICON_RESOURCE_ID as *const u16,
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
    };
    if icon.is_null() {
        return Err(Box::new(io::Error::last_os_error()));
    }

    unsafe {
        SendMessageW(window, WM_SETICON, ICON_BIG, icon as isize);
    }
    Ok(())
}

unsafe fn register_overlay_class() {
    let window_class = WndClassEx {
        size: std::mem::size_of::<WndClassEx>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        window_proc: Some(overlay_proc),
        class_extra: 0,
        window_extra: 0,
        instance: unsafe { GetModuleHandleW(ptr::null()) },
        icon: ptr::null_mut(),
        cursor: ptr::null_mut(),
        background: unsafe { GetStockObject(NULL_BRUSH) },
        menu_name: ptr::null(),
        class_name: SNAP_CLASS.as_ptr(),
        small_icon: ptr::null_mut(),
    };

    // A zero result is harmless when this process has already registered the class.
    unsafe {
        RegisterClassExW(&window_class);
    }
}

unsafe fn update_overlay_position(parent: Hwnd) {
    let state = *SNAP_STATE.lock().expect("snap state poisoned");
    let Some(state) = state.filter(|state| state.parent == parent as isize) else {
        return;
    };

    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetClientRect(parent, &mut client) } == 0 {
        return;
    }

    let dpi = unsafe { GetDpiForWindow(parent) }.max(BASE_DPI as u32) as u64;
    let (x, width, height) = overlay_bounds(client.right, dpi);
    unsafe {
        SetWindowPos(
            state.overlay as Hwnd,
            ptr::null_mut(),
            x,
            0,
            width,
            height,
            SWP_ASYNCWINDOWPOS | SWP_SHOWWINDOW,
        );
    }
}

unsafe fn remove_existing_overlay() {
    let state = SNAP_STATE.lock().expect("snap state poisoned").take();
    if let Some(state) = state {
        unsafe {
            RemoveWindowSubclass(
                state.parent as Hwnd,
                Some(parent_subclass_proc),
                SNAP_SUBCLASS_ID,
            );
            DestroyWindow(state.overlay as Hwnd);
        }
    }
}

unsafe extern "system" fn parent_subclass_proc(
    window: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    _subclass_id: usize,
    _reference_data: usize,
) -> isize {
    match message {
        WM_SIZE | WM_DPICHANGED => unsafe { update_overlay_position(window) },
        WM_CLOSE => unsafe { remove_existing_overlay() },
        _ => {}
    }

    // SAFETY: Unhandled messages must continue through the original window procedure.
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

unsafe extern "system" fn overlay_proc(
    window: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_NCHITTEST => HTMAXBUTTON,
        WM_NCLBUTTONDOWN => 0,
        WM_NCLBUTTONUP => {
            let parent = unsafe { GetParent(window) };
            if !parent.is_null() {
                let command = if unsafe { IsZoomed(parent) } != 0 {
                    SC_RESTORE
                } else {
                    SC_MAXIMIZE
                };
                unsafe {
                    PostMessageW(parent, WM_SYSCOMMAND, command, 0);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn overlay_bounds(client_right: i32, dpi: u64) -> (i32, i32, i32) {
    let width = scale_css_pixels(CAPTION_BUTTON_WIDTH_CSS, dpi).max(1);
    let height = scale_css_pixels(TITLEBAR_HEIGHT_CSS, dpi).max(1);
    (client_right - width * 2, width, height)
}

fn scale_css_pixels(value: u32, dpi: u64) -> i32 {
    ((u64::from(value) * dpi + BASE_DPI / 2) / BASE_DPI) as i32
}

#[cfg(test)]
mod tests {
    use super::{overlay_bounds, scale_css_pixels};

    #[test]
    fn scales_caption_metrics_for_window_dpi() {
        assert_eq!(scale_css_pixels(46, 96), 46);
        assert_eq!(scale_css_pixels(46, 144), 69);
        assert_eq!(scale_css_pixels(42, 192), 84);
    }

    #[test]
    fn positions_overlay_over_the_maximize_button() {
        assert_eq!(overlay_bounds(1280, 96), (1188, 46, 42));
        assert_eq!(overlay_bounds(1920, 144), (1782, 69, 63));
    }
}
