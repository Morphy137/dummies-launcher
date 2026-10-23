#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::c_void;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufRead};
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::time::Instant;

use image::RgbaImage;
use image::imageops::FilterType;
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
    HTCLIENT, ICON_BIG, ICON_SMALL, IDC_ARROW, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
    IDI_APPLICATION, KillTimer, LoadCursorW, LoadIconW, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, SIZE_MINIMIZED, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_MINIMIZE, SW_RESTORE,
    SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW,
    SetCursor, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    TranslateMessage, ULW_ALPHA, UpdateLayeredWindow, WM_APP, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_SETCURSOR, WM_SETICON, WM_SIZE,
    WM_TIMER, WNDCLASSW, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_MINIMIZEBOX,
    WS_POPUP, WS_SYSMENU,
};

const WINDOW_TITLE: &str = "Game";
const DEFAULT_WIDTH: usize = 240;
const DEFAULT_HEIGHT: usize = 270;
const MIN_WIDTH: usize = 180;
const MIN_HEIGHT: usize = 210;
const MAX_WIDTH: usize = 640;
const MAX_HEIGHT: usize = 720;
const TITLE_HEIGHT: usize = 34;
const BUTTON_SIZE: usize = 24;
const BUTTON_MARGIN: usize = 5;
const DETECTION_SIZE: usize = 64;
const ART_MASTER_SIZE: usize = 512;
const WM_APP_SHOW: u32 = WM_APP + 1;
const WM_APP_HIDE: u32 = WM_APP + 2;
const WM_APP_QUIT: u32 = WM_APP + 3;
const RESIZE_LEFT: u32 = 1;
const RESIZE_RIGHT: u32 = 2;
const RESIZE_TOP: u32 = 4;
const RESIZE_BOTTOM: u32 = 8;

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
    open_master: RgbaImage,
    closed_master: RgbaImage,
    open_frame: Vec<u8>,
    closed_frame: Vec<u8>,
    width: usize,
    height: usize,
    is_closed: bool,
    minimize_to_tray: bool,
    detection_hwnd: HWND,
    dragging: bool,
    drag_cursor: POINT,
    drag_window: POINT,
    resizing: u32,
    resize_cursor: POINT,
    resize_rect: RECT,
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

fn decode_frame(bytes: &[u8]) -> RgbaImage {
    let image = image::load_from_memory(bytes)
        .expect("dummy art frame is invalid")
        .to_rgba8();
    assert_eq!(
        image.dimensions(),
        (ART_MASTER_SIZE as u32, ART_MASTER_SIZE as u32),
        "dummy art frame has an unexpected size"
    );
    image
}

fn blend_pixel(buffer: &mut [u8], width: usize, height: usize, x: usize, y: usize, pixel: [u8; 4]) {
    if pixel[3] == 0 || x >= width || y >= height {
        return;
    }

    let source_alpha = u32::from(pixel[3]);
    let inverse = 255 - source_alpha;
    let index = (y * width + x) * 4;
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

fn art_layout(width: usize, height: usize) -> (usize, usize, usize) {
    let available_height = height.saturating_sub(TITLE_HEIGHT);
    let art_size = width.min(available_height).max(1);
    let art_x = (width - art_size) / 2;
    let art_y = TITLE_HEIGHT + (available_height - art_size) / 2;
    (art_size, art_x, art_y)
}

fn compose_frame(master: &RgbaImage, width: usize, height: usize) -> Vec<u8> {
    let (art_size, art_x, art_y) = art_layout(width, height);
    let art = image::imageops::resize(
        master,
        art_size as u32,
        art_size as u32,
        FilterType::Triangle,
    );
    let mut frame = vec![0u8; width * height * 4];

    draw_panel(&mut frame, width, height);

    for y in 0..art_size {
        for x in 0..art_size {
            let pixel = art.get_pixel(x as u32, y as u32);
            blend_pixel(
                &mut frame,
                width,
                height,
                x + art_x,
                y + art_y,
                [pixel[0], pixel[1], pixel[2], pixel[3]],
            );
        }
    }

    draw_chrome(&mut frame, width, height);
    frame
}

fn draw_panel(frame: &mut [u8], width: usize, height: usize) {
    let radius = 18;
    for y in 0..height {
        let t = if height > 1 {
            y as f32 / (height - 1) as f32
        } else {
            0.0
        };
        let base_r = (20.0 * (1.0 - t) + 8.0 * t) as u8;
        let base_g = (23.0 * (1.0 - t) + 9.0 * t) as u8;
        let base_b = (30.0 * (1.0 - t) + 12.0 * t) as u8;

        for x in 0..width {
            if !inside_rounded_rect(x, y, width, height, radius) {
                continue;
            }

            let noise = ((x * 17 + y * 31) % 7) as i16 - 3;
            let edge = if x < 2 || y < 2 || x + 2 >= width || y + 2 >= height {
                8
            } else {
                0
            };
            blend_pixel(
                frame,
                width,
                height,
                x,
                y,
                [
                    (i16::from(base_r) + noise + edge).clamp(0, 255) as u8,
                    (i16::from(base_g) + noise + edge).clamp(0, 255) as u8,
                    (i16::from(base_b) + noise + edge).clamp(0, 255) as u8,
                    198,
                ],
            );
        }
    }

    for y in 0..height {
        for x in 0..width {
            if !inside_rounded_rect(x, y, width, height, radius) {
                continue;
            }
            let border = x == 0
                || y == 0
                || x + 1 == width
                || y + 1 == height
                || !inside_rounded_rect(x.saturating_sub(1), y, width, height, radius)
                || !inside_rounded_rect(x + 1, y, width, height, radius)
                || !inside_rounded_rect(x, y.saturating_sub(1), width, height, radius)
                || !inside_rounded_rect(x, y + 1, width, height, radius);
            if border {
                blend_pixel(frame, width, height, x, y, [255, 255, 255, 20]);
            }
        }
    }

    for x in 12..width.saturating_sub(12) {
        blend_pixel(frame, width, height, x, height - 2, [255, 255, 255, 12]);
    }
}

fn draw_chrome(frame: &mut [u8], width: usize, height: usize) {
    for y in 0..TITLE_HEIGHT {
        let t = y as f32 / TITLE_HEIGHT.max(1) as f32;
        let top = [
            (38.0 * (1.0 - t) + 16.0 * t) as u8,
            (40.0 * (1.0 - t) + 18.0 * t) as u8,
            (48.0 * (1.0 - t) + 24.0 * t) as u8,
        ];
        for x in 0..width {
            if inside_rounded_top(x, y, width, TITLE_HEIGHT, 16) {
                blend_pixel(frame, width, height, x, y, [top[0], top[1], top[2], 238]);
            }
        }
    }

    for x in 12..width.saturating_sub(12) {
        blend_pixel(frame, width, height, x, 0, [255, 255, 255, 34]);
    }
    for y in TITLE_HEIGHT.saturating_sub(2)..TITLE_HEIGHT {
        for x in 14..width.saturating_sub(14) {
            blend_pixel(frame, width, height, x, y, [255, 255, 255, 22]);
        }
    }

    let close_x = width.saturating_sub(BUTTON_SIZE + BUTTON_MARGIN);
    let minimize_x = close_x.saturating_sub(BUTTON_SIZE + 4);
    let button_y = (TITLE_HEIGHT - BUTTON_SIZE) / 2;

    for x in [minimize_x, close_x] {
        for y in 0..BUTTON_SIZE {
            for offset in 0..BUTTON_SIZE {
                if inside_rounded_rect(offset, y, BUTTON_SIZE, BUTTON_SIZE, 7) {
                    blend_pixel(
                        frame,
                        width,
                        height,
                        x + offset,
                        button_y + y,
                        [255, 255, 255, 25],
                    );
                }
            }
        }
    }

    let icon_center_y = button_y + BUTTON_SIZE / 2;
    for y in icon_center_y - 1..icon_center_y + 1 {
        for x in minimize_x + 7..minimize_x + 17 {
            blend_pixel(frame, width, height, x, y, [246, 246, 248, 230]);
        }
    }

    for offset in 0..11 {
        for thickness in 0..2 {
            blend_pixel(
                frame,
                width,
                height,
                close_x + 6 + offset,
                button_y + 6 + offset + thickness,
                [246, 246, 248, 230],
            );
            blend_pixel(
                frame,
                width,
                height,
                close_x + 17 - offset,
                button_y + 6 + offset + thickness,
                [246, 246, 248, 230],
            );
        }
    }

    let brand_x = 12;
    let brand_y = (TITLE_HEIGHT - 12) / 2;
    for y in 0..12 {
        for x in 0..12 {
            if inside_rounded_rect(x, y, 12, 12, 4) {
                blend_pixel(
                    frame,
                    width,
                    height,
                    brand_x + x,
                    brand_y + y,
                    [198, 31, 91, 245],
                );
            }
        }
    }
    for y in 4..7 {
        for x in 4..8 {
            blend_pixel(
                frame,
                width,
                height,
                brand_x + x,
                brand_y + y,
                [255, 255, 255, 210],
            );
        }
    }

    for dot in [width / 2 - 7, width / 2, width / 2 + 7] {
        for y in 16..18 {
            for x in dot..dot + 2 {
                blend_pixel(frame, width, height, x, y, [255, 255, 255, 62]);
            }
        }
    }

    // A nearly invisible border gives the layered window a hit-test surface on
    // all four sides, so it can be resized from any edge.
    let hit_border = 6;
    for x in 0..width {
        for y in 0..hit_border {
            blend_pixel(frame, width, height, x, y, [7, 8, 10, 1]);
            blend_pixel(frame, width, height, x, height - 1 - y, [7, 8, 10, 1]);
        }
    }
    for y in 0..height {
        for x in 0..hit_border {
            blend_pixel(frame, width, height, x, y, [7, 8, 10, 1]);
            blend_pixel(frame, width, height, width - 1 - x, y, [7, 8, 10, 1]);
        }
    }
}

impl WindowState {
    fn new(open_master: RgbaImage, closed_master: RgbaImage, minimize_to_tray: bool) -> Self {
        unsafe {
            let screen_dc = GetDC(null_mut());
            let mem_dc = CreateCompatibleDC(screen_dc);
            let mut state = Self {
                open_master,
                closed_master,
                open_frame: Vec::new(),
                closed_frame: Vec::new(),
                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,
                is_closed: false,
                minimize_to_tray,
                detection_hwnd: null_mut(),
                dragging: false,
                drag_cursor: POINT { x: 0, y: 0 },
                drag_window: POINT { x: 0, y: 0 },
                resizing: 0,
                resize_cursor: POINT { x: 0, y: 0 },
                resize_rect: zeroed(),
                screen_dc,
                mem_dc,
                bitmap: null_mut(),
                old_bitmap: null_mut(),
                bits: null_mut(),
            };
            state.rebuild_frames();
            state.create_surface();
            state
        }
    }

    fn rebuild_frames(&mut self) {
        self.open_frame = compose_frame(&self.open_master, self.width, self.height);
        self.closed_frame = compose_frame(&self.closed_master, self.width, self.height);
    }

    fn create_surface(&mut self) {
        unsafe {
            if !self.bitmap.is_null() {
                SelectObject(self.mem_dc, self.old_bitmap);
                DeleteObject(self.bitmap.cast());
                self.bitmap = null_mut();
                self.bits = null_mut();
            }

            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.width as i32,
                biHeight: -(self.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..zeroed()
            };
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(
                self.screen_dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                null_mut(),
                0,
            );
            assert!(
                !bitmap.is_null(),
                "could not create the layered window bitmap"
            );
            self.old_bitmap = SelectObject(self.mem_dc, bitmap);
            self.bitmap = bitmap;
            self.bits = bits.cast();
        }
    }

    fn resize(&mut self, hwnd: HWND, width: usize, height: usize) {
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        let height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);
        if width == self.width && height == self.height {
            return;
        }

        self.width = width;
        self.height = height;
        self.rebuild_frames();
        self.create_surface();
        self.present(hwnd);
    }

    fn present(&self, hwnd: HWND) {
        let frame = if self.is_closed {
            &self.closed_frame
        } else {
            &self.open_frame
        };

        unsafe {
            std::ptr::copy_nonoverlapping(frame.as_ptr(), self.bits, frame.len());
            let size = SIZE {
                cx: self.width as i32,
                cy: self.height as i32,
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
            if !self.bitmap.is_null() {
                SelectObject(self.mem_dc, self.old_bitmap);
                DeleteObject(self.bitmap.cast());
            }
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

    let (art_size, art_x, art_y) = art_layout(state.width, state.height);
    let helper_x = art_x + art_size / 2 - DETECTION_SIZE / 2;
    let helper_y = art_y + art_size / 2 - DETECTION_SIZE / 2;
    let mut rect: RECT = unsafe { zeroed() };
    unsafe {
        GetWindowRect(main_window, &mut rect);
        SetWindowPos(
            state.detection_hwnd,
            main_window,
            rect.left + helper_x as i32,
            rect.top + helper_y as i32,
            DETECTION_SIZE as i32,
            DETECTION_SIZE as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

fn resize_edge(x: i32, y: i32, width: usize, height: usize) -> u32 {
    let border = 6;
    let mut edge = 0;
    if x < border {
        edge |= RESIZE_LEFT;
    } else if x >= width as i32 - border {
        edge |= RESIZE_RIGHT;
    }
    if y < border {
        edge |= RESIZE_TOP;
    } else if y >= height as i32 - border {
        edge |= RESIZE_BOTTOM;
    }
    edge
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
                    let close_x = state.width as i32 - BUTTON_SIZE as i32 - BUTTON_MARGIN as i32;
                    let minimize_x = close_x - BUTTON_SIZE as i32 - 4;
                    let button_y = ((TITLE_HEIGHT - BUTTON_SIZE) / 2) as i32;
                    let button_bottom = button_y + BUTTON_SIZE as i32;

                    if (minimize_x..minimize_x + BUTTON_SIZE as i32).contains(&x)
                        && (button_y..button_bottom).contains(&y)
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
                    if (close_x..close_x + BUTTON_SIZE as i32).contains(&x)
                        && (button_y..button_bottom).contains(&y)
                    {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                        return 0;
                    }

                    let edge = resize_edge(x, y, state.width, state.height);
                    if edge != 0 {
                        state.resizing = edge;
                        GetCursorPos(&mut state.resize_cursor);
                        GetWindowRect(hwnd, &mut state.resize_rect);
                        SetCapture(hwnd);
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
                    let mut cursor = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut cursor);

                    if state.resizing != 0 {
                        let dx = cursor.x - state.resize_cursor.x;
                        let dy = cursor.y - state.resize_cursor.y;
                        let mut rect = state.resize_rect;

                        if state.resizing & RESIZE_LEFT != 0 {
                            rect.left += dx;
                        }
                        if state.resizing & RESIZE_RIGHT != 0 {
                            rect.right += dx;
                        }
                        if state.resizing & RESIZE_TOP != 0 {
                            rect.top += dy;
                        }
                        if state.resizing & RESIZE_BOTTOM != 0 {
                            rect.bottom += dy;
                        }

                        if rect.right - rect.left < MIN_WIDTH as i32 {
                            if state.resizing & RESIZE_LEFT != 0 {
                                rect.left = rect.right - MIN_WIDTH as i32;
                            } else {
                                rect.right = rect.left + MIN_WIDTH as i32;
                            }
                        }
                        if rect.bottom - rect.top < MIN_HEIGHT as i32 {
                            if state.resizing & RESIZE_TOP != 0 {
                                rect.top = rect.bottom - MIN_HEIGHT as i32;
                            } else {
                                rect.bottom = rect.top + MIN_HEIGHT as i32;
                            }
                        }
                        if rect.right - rect.left > MAX_WIDTH as i32 {
                            if state.resizing & RESIZE_LEFT != 0 {
                                rect.left = rect.right - MAX_WIDTH as i32;
                            } else {
                                rect.right = rect.left + MAX_WIDTH as i32;
                            }
                        }
                        if rect.bottom - rect.top > MAX_HEIGHT as i32 {
                            if state.resizing & RESIZE_TOP != 0 {
                                rect.top = rect.bottom - MAX_HEIGHT as i32;
                            } else {
                                rect.bottom = rect.top + MAX_HEIGHT as i32;
                            }
                        }

                        let width = rect.right - rect.left;
                        let height = rect.bottom - rect.top;
                        SetWindowPos(
                            hwnd,
                            null_mut(),
                            rect.left,
                            rect.top,
                            width,
                            height,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    } else if state.dragging {
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
                    (*state).resizing = 0;
                }
                ReleaseCapture();
                0
            }
            WM_SETCURSOR => {
                if (lparam & 0xffff) as u32 == HTCLIENT {
                    let state = state_ptr(hwnd);
                    if !state.is_null() {
                        let mut cursor = POINT { x: 0, y: 0 };
                        let mut rect: RECT = zeroed();
                        GetCursorPos(&mut cursor);
                        GetWindowRect(hwnd, &mut rect);
                        let edge = resize_edge(
                            cursor.x - rect.left,
                            cursor.y - rect.top,
                            (*state).width,
                            (*state).height,
                        );
                        let cursor_id = match edge {
                            edge if edge == (RESIZE_LEFT | RESIZE_TOP) => IDC_SIZENWSE,
                            edge if edge == (RESIZE_RIGHT | RESIZE_BOTTOM) => IDC_SIZENWSE,
                            edge if edge == (RESIZE_RIGHT | RESIZE_TOP) => IDC_SIZENESW,
                            edge if edge == (RESIZE_LEFT | RESIZE_BOTTOM) => IDC_SIZENESW,
                            edge if edge & (RESIZE_LEFT | RESIZE_RIGHT) != 0 => IDC_SIZEWE,
                            edge if edge & (RESIZE_TOP | RESIZE_BOTTOM) != 0 => IDC_SIZENS,
                            _ => IDC_ARROW,
                        };
                        SetCursor(LoadCursorW(null_mut(), cursor_id));
                        return 1;
                    }
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
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
                        let width = (lparam & 0xffff) as usize;
                        let height = ((lparam >> 16) & 0xffff) as usize;
                        if width > 0 && height > 0 {
                            (*state).resize(hwnd, width, height);
                            position_detection_window(&*state, hwnd);
                        }
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
    let mut random = std::collections::hash_map::RandomState::new().build_hasher();
    random.write_u64(u64::from(std::process::id()));
    random.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default(),
    );
    let art_index = (random.finish() as usize) % ART_PAIRS.len();
    let (open_bytes, closed_bytes) = ART_PAIRS[art_index];
    let open = decode_frame(open_bytes);
    let closed = decode_frame(closed_bytes);
    let _ = START.set(Instant::now());

    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = class_name_wide();
        let mut window_class: WNDCLASSW = zeroed();
        window_class.style = CS_HREDRAW | CS_VREDRAW;
        window_class.lpfnWndProc = Some(window_proc);
        window_class.hInstance = instance;
        let icon_name = std::ptr::without_provenance::<u16>(1);
        let icon = LoadIconW(instance, icon_name);
        let icon = if icon.is_null() {
            LoadIconW(null_mut(), IDI_APPLICATION)
        } else {
            icon
        };
        window_class.hIcon = icon;
        window_class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        window_class.lpszClassName = class_name.as_ptr();
        RegisterClassW(&window_class);

        let title = wide(&options.display_name);
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_width - DEFAULT_WIDTH as i32) / 2;
        let y = (screen_height - DEFAULT_HEIGHT as i32) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_APPWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU,
            x,
            y,
            DEFAULT_WIDTH as i32,
            DEFAULT_HEIGHT as i32,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );

        if hwnd.is_null() {
            std::process::exit(1);
        }

        SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, icon as LPARAM);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as WPARAM, icon as LPARAM);

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
            0,
            0,
            DETECTION_SIZE as i32,
            DETECTION_SIZE as i32,
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
    use super::{
        ART_MASTER_SIZE, DEFAULT_HEIGHT, DEFAULT_WIDTH, TITLE_HEIGHT, art_layout, compose_frame,
        inside_rounded_rect, parse_options,
    };
    use image::RgbaImage;

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

        let master = RgbaImage::new(ART_MASTER_SIZE as u32, ART_MASTER_SIZE as u32);
        let frame = compose_frame(&master, DEFAULT_WIDTH, DEFAULT_HEIGHT);
        assert!(frame.len() == DEFAULT_WIDTH * DEFAULT_HEIGHT * 4);
    }

    #[test]
    fn centers_square_art_in_resized_window() {
        let default_art_size = DEFAULT_WIDTH.min(DEFAULT_HEIGHT - TITLE_HEIGHT);
        assert_eq!(
            art_layout(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            (
                default_art_size,
                (DEFAULT_WIDTH - default_art_size) / 2,
                TITLE_HEIGHT
            )
        );

        let (art_size, art_x, art_y) = art_layout(400, 500);
        assert_eq!(art_size, 400);
        assert_eq!(art_x, 0);
        assert_eq!(art_y, TITLE_HEIGHT + (500 - TITLE_HEIGHT - 400) / 2);

        let master = RgbaImage::new(ART_MASTER_SIZE as u32, ART_MASTER_SIZE as u32);
        let frame = compose_frame(&master, 400, 500);
        assert_eq!(frame.len(), 400 * 500 * 4);
    }
}
