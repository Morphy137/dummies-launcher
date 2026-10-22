#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{self, BufRead};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DEFAULT_GUI_FONT, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, EndPaint, FillRect, GetStockObject, PAINTSTRUCT, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetClientRect, GetMessageW, IDC_ARROW, LoadCursorW, MSG, PostMessageW,
    PostQuitMessage, RegisterClassW, SIZE_MINIMIZED, SW_HIDE, SW_SHOW, SetForegroundWindow,
    ShowWindow, TranslateMessage, WM_APP, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WM_SIZE,
    WNDCLASSW, WS_CAPTION, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};

const WM_APP_SHOW: u32 = WM_APP + 1;
const WM_APP_HIDE: u32 = WM_APP + 2;
const WM_APP_QUIT: u32 = WM_APP + 3;

const CLASS_NAME: &str = "DummiesDummyWindow";
const WINDOW_TITLE: &str = "Game";

static DISPLAY_NAME: OnceLock<String> = OnceLock::new();
static CLASS_NAME_WIDE: OnceLock<Vec<u16>> = OnceLock::new();
static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(true);
static MESSAGE_HWND: AtomicIsize = AtomicIsize::new(0);
static VISIBLE_HWND: AtomicIsize = AtomicIsize::new(0);

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn class_name_wide() -> &'static [u16] {
    CLASS_NAME_WIDE.get_or_init(|| wide(CLASS_NAME)).as_slice()
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16)
}

struct Options {
    display_name: String,
    show_window: bool,
    minimize_to_tray: bool,
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Options {
    let mut display_name = WINDOW_TITLE.to_string();
    let mut show_window = false;
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

fn post_message(message: u32) {
    let hwnd = MESSAGE_HWND.load(Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        unsafe {
            PostMessageW(hwnd, message, 0, 0);
        }
    }
}

fn handle_command(line: &str) {
    let command = line.trim();

    if command.eq_ignore_ascii_case("SHOW") {
        post_message(WM_APP_SHOW);
    } else if command.eq_ignore_ascii_case("HIDE") {
        post_message(WM_APP_HIDE);
    } else if command.eq_ignore_ascii_case("QUIT") || command.eq_ignore_ascii_case("STOP") {
        post_message(WM_APP_QUIT);
    }
}

unsafe fn show_visible_window() {
    let existing = VISIBLE_HWND.load(Ordering::SeqCst) as HWND;
    if !existing.is_null() {
        unsafe {
            ShowWindow(existing, SW_SHOW);
            SetForegroundWindow(existing);
        }
        return;
    }

    let display_name = DISPLAY_NAME
        .get()
        .map(String::as_str)
        .unwrap_or(WINDOW_TITLE);
    let title = wide(display_name);
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name_wide().as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            200,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };

    if hwnd.is_null() {
        return;
    }

    VISIBLE_HWND.store(hwnd as isize, Ordering::SeqCst);
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

unsafe fn hide_visible_window() {
    let hwnd = VISIBLE_HWND.swap(0, Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}

unsafe fn paint_window(hwnd: HWND) {
    let mut paint: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let device_context = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client_rect: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut client_rect);
    }

    let background = unsafe { CreateSolidBrush(rgb(7, 8, 10)) };
    unsafe {
        FillRect(device_context, &client_rect, background);
        DeleteObject(background);
        SetBkMode(device_context, TRANSPARENT as i32);
        SetTextColor(device_context, rgb(244, 244, 246));
    }

    let font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };
    let previous_font = unsafe { SelectObject(device_context, font) };

    let display_name = DISPLAY_NAME
        .get()
        .map(String::as_str)
        .unwrap_or(WINDOW_TITLE);
    let title = wide(display_name);
    let mut title_rect = client_rect;
    title_rect.top += 36;
    title_rect.bottom -= 48;

    unsafe {
        DrawTextW(
            device_context,
            title.as_ptr(),
            -1,
            &mut title_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SetTextColor(device_context, rgb(156, 156, 157));
    }

    let subtitle = wide("Dummy process");
    let mut subtitle_rect = client_rect;
    subtitle_rect.top = client_rect.bottom - 44;
    unsafe {
        DrawTextW(
            device_context,
            subtitle.as_ptr(),
            -1,
            &mut subtitle_rect,
            DT_CENTER | DT_SINGLELINE,
        );
        SelectObject(device_context, previous_font);
        EndPaint(hwnd, &paint);
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_APP_SHOW => {
            unsafe {
                show_visible_window();
            }
            0
        }
        WM_APP_HIDE => {
            unsafe {
                hide_visible_window();
            }
            0
        }
        WM_APP_QUIT => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        WM_CLOSE => {
            let message_hwnd = MESSAGE_HWND.load(Ordering::SeqCst) as HWND;
            if hwnd == message_hwnd {
                unsafe {
                    PostQuitMessage(0);
                }
            } else {
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
            0
        }
        WM_DESTROY => {
            let message_hwnd = MESSAGE_HWND.load(Ordering::SeqCst) as HWND;
            if hwnd == message_hwnd {
                unsafe {
                    PostQuitMessage(0);
                }
            } else {
                VISIBLE_HWND.store(0, Ordering::SeqCst);
            }
            0
        }
        WM_SIZE => {
            if wparam as u32 == SIZE_MINIMIZED && MINIMIZE_TO_TRAY.load(Ordering::SeqCst) {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            unsafe {
                paint_window(hwnd);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn create_message_window() -> Option<HWND> {
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let mut window_class: WNDCLASSW = unsafe { std::mem::zeroed() };
    window_class.style = CS_HREDRAW | CS_VREDRAW;
    window_class.lpfnWndProc = Some(window_proc);
    window_class.hInstance = instance;
    window_class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    window_class.hbrBackground = unsafe { CreateSolidBrush(rgb(7, 8, 10)) };
    window_class.lpszClassName = class_name_wide().as_ptr();

    let registered = unsafe { RegisterClassW(&window_class) };
    if registered == 0 {
        return None;
    }

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name_wide().as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            windows_sys::Win32::UI::WindowsAndMessaging::HWND_MESSAGE,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };

    if hwnd.is_null() { None } else { Some(hwnd) }
}

fn start_stdin_listener() {
    std::thread::spawn(|| {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => handle_command(&line),
            }
        }

        post_message(WM_APP_QUIT);
    });
}

fn main() {
    let options = parse_options(std::env::args().skip(1));
    let _ = DISPLAY_NAME.set(options.display_name);
    MINIMIZE_TO_TRAY.store(options.minimize_to_tray, Ordering::SeqCst);

    let message_window = unsafe { create_message_window() };
    let Some(message_window) = message_window else {
        std::process::exit(1);
    };

    MESSAGE_HWND.store(message_window as isize, Ordering::SeqCst);
    start_stdin_listener();

    if options.show_window {
        unsafe {
            show_visible_window();
        }
    }

    let mut message: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_options, rgb};

    #[test]
    fn parses_display_name_and_flags() {
        let options = parse_options([
            "Helldivers 2".to_string(),
            "--window".to_string(),
            "--no-tray".to_string(),
        ]);

        assert_eq!(options.display_name, "Helldivers 2");
        assert!(options.show_window);
        assert!(!options.minimize_to_tray);
    }

    #[test]
    fn defaults_to_hidden_with_tray_behavior() {
        let options = parse_options(Vec::<String>::new());

        assert_eq!(options.display_name, "Game");
        assert!(!options.show_window);
        assert!(options.minimize_to_tray);
    }

    #[test]
    fn builds_windows_colorref() {
        assert_eq!(rgb(7, 8, 10), 0x000a_0807);
    }
}
