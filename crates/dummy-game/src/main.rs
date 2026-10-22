#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::c_void;
use std::io::{self, BufRead};
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::time::Instant;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP,
    HDC, HGDIOBJ, ReleaseDC, SelectObject, ValidateRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
    IDC_ARROW, KillTimer, LoadCursorW, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    SIZE_MINIMIZED, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow,
    SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA,
    UpdateLayeredWindow, WM_APP, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WM_SIZE, WM_TIMER, WNDCLASSW, WS_EX_APPWINDOW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
};

const WINDOW_TITLE: &str = "Game";
const WINDOW_WIDTH: usize = 320;
const WINDOW_HEIGHT: usize = 356;
const ART_TOP: usize = 36;
const ART_SIZE: usize = 320;
const WM_APP_SHOW: u32 = WM_APP + 1;
const WM_APP_HIDE: u32 = WM_APP + 2;
const WM_APP_QUIT: u32 = WM_APP + 3;

const ART_PAIRS: &[(&[u8], &[u8])] = &[
    (
        include_bytes!("../assets/frames/mood-01-open.png"),
        include_bytes!("../assets/frames/mood-01-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-02-open.png"),
        include_bytes!("../assets/frames/mood-02-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-03-open.png"),
        include_bytes!("../assets/frames/mood-03-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-04-open.png"),
        include_bytes!("../assets/frames/mood-04-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-05-open.png"),
        include_bytes!("../assets/frames/mood-05-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-06-open.png"),
        include_bytes!("../assets/frames/mood-06-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-07-open.png"),
        include_bytes!("../assets/frames/mood-07-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-08-open.png"),
        include_bytes!("../assets/frames/mood-08-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-09-open.png"),
        include_bytes!("../assets/frames/mood-09-closed.png"),
    ),
    (
        include_bytes!("../assets/frames/mood-10-open.png"),
        include_bytes!("../assets/frames/mood-10-closed.png"),
    ),
];

static START: OnceLock<Instant> = OnceLock::new();
static CLASS_NAME_WIDE: OnceLock<Vec<u16>> = OnceLock::new();

struct Options {
    display_name: String,
    show_window: bool,
    minimize_to_tray: bool,
}

struct WindowState {
    open: Vec<u8>,
    closed: Vec<u8>,
    is_closed: bool,
    minimize_to_tray: bool,
    detection_hwnd: HWND,
    dragging: bool,
    drag_cursor: POINT,
    drag_window: POINT,
    screen_dc: HDC,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    bits: *mut u8,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn class_name_wide() -> &'static [u16] {
    CLASS_NAME_WIDE
        .get_or_init(|| wide("DummiesDummyLayeredWindow"))
        .as_slice()
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Options {
    let mut display_name = WINDOW_TITLE.to_string();
    let mut show_window = true;
    let mut minimize_to_tray = true;
    let mut first_value = true;

    for arg in args {
        let value = arg.trim();
        if value.is_empty() {
            continue;
        }

        if value.eq_ignore_ascii_case("--window") || value.eq_ignore_ascii_case("--visible") {
            show_window = true;
        } else if value.eq_ignore_ascii_case("--hidden")
            || value.eq_ignore_ascii_case("--start-hidden")
        {
            show_window = false;
        } else if value.eq_ignore_ascii_case("--no-tray") {
            minimize_to_tray = false;
        } else if first_value && !value.starts_with("--") {
            display_name = value.to_string();
        }

        first_value = false;
    }

    Options {
        display_name,
        show_window,
        minimize_to_tray,
    }
}

fn decode_frame(bytes: &[u8]) -> Vec<u8> {
    let image = image::load_from_memory(bytes)
        .expect("dummy art frame is invalid")
        .to_rgba8();
    assert_eq!(
        image.dimensions(),
        (ART_SIZE as u32, ART_SIZE as u32),
        "dummy art frame has an unexpected size"
    );
    image.into_raw()
}

fn premultiply(pixel: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(pixel[3]);
    [
        ((u32::from(pixel[0]) * alpha + 127) / 255) as u8,
        ((u32::from(pixel[1]) * alpha + 127) / 255) as u8,
        ((u32::from(pixel[2]) * alpha + 127) / 255) as u8,
        pixel[3],
    ]
}

fn blend_pixel(buffer: &mut [u8], x: usize, y: usize, pixel: [u8; 4]) {
    if pixel[3] == 0 || x >= WINDOW_WIDTH || y >= WINDOW_HEIGHT {
        return;
    }

    let source_alpha = u32::from(pixel[3]);
    let inverse = 255 - source_alpha;
    let index = (y * WINDOW_WIDTH + x) * 4;
    let source_b = (u32::from(pixel[2]) * source_alpha + 127) / 255;
    let source_g = (u32::from(pixel[1]) * source_alpha + 127) / 255;
    let source_r = (u32::from(pixel[0]) * source_alpha + 127) / 255;
    let destination_alpha = u32::from(buffer[index + 3]);

    buffer[index] = (source_b + (u32::from(buffer[index]) * inverse + 127) / 255) as u8;
    buffer[index + 1] = (source_g + (u32::from(buffer[index + 1]) * inverse + 127) / 255) as u8;
    buffer[index + 2] = (source_r + (u32::from(buffer[index + 2]) * inverse + 127) / 255) as u8;
    buffer[index + 3] = (source_alpha + (destination_alpha * inverse + 127) / 255).min(255) as u8;
}

fn inside_rounded_rect(x: usize, y: usize, width: usize, height: usize, radius: usize) -> bool {
    if x >= width || y >= height {
        return false;
    }

    let corner_x = if x < radius {
        radius - 1 - x
    } else if x >= width.saturating_sub(radius) {
        x - (width - radius)
    } else {
        return true;
    };
    let corner_y = if y < radius {
        radius - 1 - y
    } else if y >= height.saturating_sub(radius) {
        y - (height - radius)
    } else {
        return true;
    };

    corner_x * corner_x + corner_y * corner_y <= radius * radius
}

fn inside_rounded_top(x: usize, y: usize, width: usize, height: usize, radius: usize) -> bool {
    if y >= height {
        return false;
    }
    if y >= radius {
        return true;
    }
    if x < radius {
        let dx = radius - 1 - x;
        let dy = radius - 1 - y;
        return dx * dx + dy * dy <= radius * radius;
    }
    if x >= width.saturating_sub(radius) {
        let dx = x - (width - radius);
        let dy = radius - 1 - y;
        return dx * dx + dy * dy <= radius * radius;
    }
    true
}

fn draw_frame(art: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; WINDOW_WIDTH * WINDOW_HEIGHT * 4];

    for y in 0..ART_SIZE {
        for x in 0..ART_SIZE {
            let index = (y * ART_SIZE + x) * 4;
            let pixel = premultiply([art[index], art[index + 1], art[index + 2], art[index + 3]]);
            let target = ((y + ART_TOP) * WINDOW_WIDTH + x) * 4;
            frame[target..target + 4].copy_from_slice(&pixel);
        }
    }

    for y in 0..34 {
        for x in 0..WINDOW_WIDTH {
            if inside_rounded_top(x, y, WINDOW_WIDTH, 34, 10) {
                blend_pixel(&mut frame, x, y, [7, 8, 10, 150]);
            }
        }
    }

    for y in 32..34 {
        for x in 10..WINDOW_WIDTH - 10 {
            blend_pixel(&mut frame, x, y, [255, 255, 255, 18]);
        }
    }

    let minimize_x = WINDOW_WIDTH - 58;
    let close_x = WINDOW_WIDTH - 30;
    for x in [minimize_x, close_x] {
        for y in 5..29 {
            for offset in 0..24 {
                if inside_rounded_rect(offset, y - 5, 24, 24, 6) {
                    blend_pixel(&mut frame, x + offset, y, [255, 255, 255, 22]);
                }
            }
        }
    }

    for y in 16..18 {
        for x in minimize_x + 7..minimize_x + 17 {
            blend_pixel(&mut frame, x, y, [244, 244, 246, 225]);
        }
    }

    for offset in 0..9 {
        for thickness in 0..2 {
            blend_pixel(
                &mut frame,
                close_x + 7 + offset,
                7 + offset + thickness,
                [244, 244, 246, 225],
            );
            blend_pixel(
                &mut frame,
                close_x + 16 - offset,
                7 + offset + thickness,
                [244, 244, 246, 225],
            );
        }
    }

    frame
}

impl WindowState {
    fn new(open: Vec<u8>, closed: Vec<u8>, minimize_to_tray: bool) -> Self {
        unsafe {
            let screen_dc = GetDC(null_mut());
            let mem_dc = CreateCompatibleDC(screen_dc);
            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: WINDOW_WIDTH as i32,
                biHeight: -(WINDOW_HEIGHT as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..zeroed()
            };
            let mut bits: *mut c_void = null_mut();
            let bitmap =
                CreateDIBSection(screen_dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            assert!(
                !bitmap.is_null(),
                "could not create the layered window bitmap"
            );
            let old_bitmap = SelectObject(mem_dc, bitmap);

            Self {
                open,
                closed,
                is_closed: false,
                minimize_to_tray,
                detection_hwnd: null_mut(),
                dragging: false,
                drag_cursor: POINT { x: 0, y: 0 },
                drag_window: POINT { x: 0, y: 0 },
                screen_dc,
                mem_dc,
                bitmap,
                old_bitmap,
                bits: bits.cast(),
            }
        }
    }

    fn present(&self, hwnd: HWND) {
        let frame = if self.is_closed {
            &self.closed
        } else {
            &self.open
        };

        unsafe {
            std::ptr::copy_nonoverlapping(frame.as_ptr(), self.bits, frame.len());
            let size = SIZE {
                cx: WINDOW_WIDTH as i32,
                cy: WINDOW_HEIGHT as i32,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd,
                self.screen_dc,
                std::ptr::null(),
                &size,
                self.mem_dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );
        }
    }

    fn toggle_blink(&mut self, hwnd: HWND) {
        let elapsed = START
            .get()
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or_default();
        let closed = elapsed % 4600 >= 4200;
        if closed != self.is_closed {
            self.is_closed = closed;
            self.present(hwnd);
        }
    }
}

impl Drop for WindowState {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem_dc, self.old_bitmap);
            DeleteObject(self.bitmap.cast());
            DeleteDC(self.mem_dc);
            ReleaseDC(null_mut(), self.screen_dc);
        }
    }
}

fn state_ptr(hwnd: HWND) -> *mut WindowState {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState }
}

fn position_detection_window(state: &WindowState, main_window: HWND) {
    if state.detection_hwnd.is_null() {
        return;
    }

    let mut rect: RECT = unsafe { zeroed() };
    unsafe {
        GetWindowRect(main_window, &mut rect);
        SetWindowPos(
            state.detection_hwnd,
            main_window,
            rect.left + 128,
            rect.top + 164,
            64,
            64,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_LBUTTONDOWN => {
                let x = (lparam & 0xffff) as u16 as i16 as i32;
                let y = ((lparam >> 16) & 0xffff) as u16 as i16 as i32;
                let state = state_ptr(hwnd);

                if !state.is_null() {
                    let state = &mut *state;
                    if (WINDOW_WIDTH as i32 - 58..WINDOW_WIDTH as i32 - 34).contains(&x)
                        && (5..29).contains(&y)
                    {
                        if !state.detection_hwnd.is_null() {
                            ShowWindow(state.detection_hwnd, SW_HIDE);
                        }
                        if state.minimize_to_tray {
                            ShowWindow(hwnd, SW_HIDE);
                        } else {
                            ShowWindow(hwnd, SW_MINIMIZE);
                        }
                        return 0;
                    }
                    if (WINDOW_WIDTH as i32 - 30..WINDOW_WIDTH as i32 - 6).contains(&x)
                        && (5..29).contains(&y)
                    {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                        return 0;
                    }

                    state.dragging = true;
                    GetCursorPos(&mut state.drag_cursor);
                    let mut rect: RECT = zeroed();
                    GetWindowRect(hwnd, &mut rect);
                    state.drag_window = POINT {
                        x: rect.left,
                        y: rect.top,
                    };
                    SetCapture(hwnd);
                }
                0
            }
            WM_MOUSEMOVE => {
                let state = state_ptr(hwnd);
                if !state.is_null() {
                    let state = &mut *state;
                    if state.dragging {
                        let mut cursor = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut cursor);
                        let x = state.drag_window.x + cursor.x - state.drag_cursor.x;
                        let y = state.drag_window.y + cursor.y - state.drag_cursor.y;
                        SetWindowPos(hwnd, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
                        position_detection_window(state, hwnd);
                    }
                }
                0
            }
            WM_LBUTTONUP => {
                let state = state_ptr(hwnd);
                if !state.is_null() {
                    (*state).dragging = false;
                }
                ReleaseCapture();
                0
            }
            WM_TIMER => {
                let state = state_ptr(hwnd);
                if !state.is_null() {
                    (*state).toggle_blink(hwnd);
                }
                0
            }
            WM_SIZE => {
                let state = state_ptr(hwnd);
                if !state.is_null() {
                    if wparam as u32 == SIZE_MINIMIZED {
                        if !(*state).detection_hwnd.is_null() {
                            ShowWindow((*state).detection_hwnd, SW_HIDE);
                        }
                        if (*state).minimize_to_tray {
                            ShowWindow(hwnd, SW_HIDE);
                        }
                    } else {
                        position_detection_window(&*state, hwnd);
                    }
                }
                0
            }
            WM_APP_SHOW => {
                ShowWindow(hwnd, SW_RESTORE);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
                let state = state_ptr(hwnd);
                if !state.is_null() {
                    position_detection_window(&*state, hwnd);
                }
                SetForegroundWindow(hwnd);
                0
            }
            WM_APP_HIDE => {
                let state = state_ptr(hwnd);
                if !state.is_null() && !(*state).detection_hwnd.is_null() {
                    ShowWindow((*state).detection_hwnd, SW_HIDE);
                }
                ShowWindow(hwnd, SW_HIDE);
                0
            }
            WM_APP_QUIT => {
                PostQuitMessage(0);
                0
            }
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                ValidateRect(hwnd, std::ptr::null());
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                let state = state_ptr(hwnd);
                if !state.is_null() {
                    if !(*state).detection_hwnd.is_null() {
                        DestroyWindow((*state).detection_hwnd);
                    }
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(state));
                    PostQuitMessage(0);
                }
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

fn start_stdin_listener(hwnd: HWND) {
    let window = hwnd as isize;
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let command = line.trim();
                    if command.eq_ignore_ascii_case("SHOW") {
                        unsafe {
                            PostMessageW(window as HWND, WM_APP_SHOW, 0, 0);
                        }
                    } else if command.eq_ignore_ascii_case("HIDE") {
                        unsafe {
                            PostMessageW(window as HWND, WM_APP_HIDE, 0, 0);
                        }
                    } else if command.eq_ignore_ascii_case("QUIT")
                        || command.eq_ignore_ascii_case("STOP")
                    {
                        unsafe {
                            PostMessageW(window as HWND, WM_APP_QUIT, 0, 0);
                        }
                        break;
                    }
                }
            }
        }

        unsafe {
            PostMessageW(window as HWND, WM_APP_QUIT, 0, 0);
        }
    });
}

fn main() {
    let options = parse_options(std::env::args().skip(1));
    let art_index = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as usize)
        .unwrap_or_default()
        % ART_PAIRS.len();
    let (open_bytes, closed_bytes) = ART_PAIRS[art_index];
    let open = draw_frame(&decode_frame(open_bytes));
    let closed = draw_frame(&decode_frame(closed_bytes));
    let _ = START.set(Instant::now());

    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = class_name_wide();
        let mut window_class: WNDCLASSW = zeroed();
        window_class.style = CS_HREDRAW | CS_VREDRAW;
        window_class.lpfnWndProc = Some(window_proc);
        window_class.hInstance = instance;
        window_class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        window_class.lpszClassName = class_name.as_ptr();
        RegisterClassW(&window_class);

        let title = wide(&options.display_name);
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_width - WINDOW_WIDTH as i32) / 2;
        let y = (screen_height - WINDOW_HEIGHT as i32) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_APPWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU,
            x,
            y,
            WINDOW_WIDTH as i32,
            WINDOW_HEIGHT as i32,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );

        if hwnd.is_null() {
            std::process::exit(1);
        }

        let state = Box::new(WindowState::new(open, closed, options.minimize_to_tray));
        let state = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        (*state).present(hwnd);
        SetTimer(hwnd, 1, 50, None);
        start_stdin_listener(hwnd);

        // Discord's runner scans normal top-level windows. Keep a small normal
        // detection window underneath an opaque part of the face.
        let detection_window = CreateWindowExW(
            WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            x + 128,
            y + 164,
            64,
            64,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if !detection_window.is_null() {
            (*state).detection_hwnd = detection_window;
        }

        if options.show_window {
            ShowWindow(hwnd, SW_SHOW);
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            position_detection_window(&*state, hwnd);
            SetForegroundWindow(hwnd);
        }

        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{draw_frame, inside_rounded_rect, parse_options};

    #[test]
    fn parses_display_name_and_window_flags() {
        let options = parse_options([
            "Helldivers 2".to_string(),
            "--hidden".to_string(),
            "--no-tray".to_string(),
        ]);

        assert_eq!(options.display_name, "Helldivers 2");
        assert!(!options.show_window);
        assert!(!options.minimize_to_tray);
    }

    #[test]
    fn defaults_to_visible_with_tray_behavior() {
        let options = parse_options(Vec::<String>::new());

        assert_eq!(options.display_name, "Game");
        assert!(options.show_window);
        assert!(options.minimize_to_tray);
    }

    #[test]
    fn uses_rounded_corners_and_transparent_frame_edges() {
        assert!(!inside_rounded_rect(0, 0, 24, 24, 6));
        assert!(inside_rounded_rect(12, 12, 24, 24, 6));

        let frame = draw_frame(&vec![0; 320 * 320 * 4]);
        assert_eq!(&frame[0..4], &[0, 0, 0, 0]);
    }
}
