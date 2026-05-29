use std::path::Path;
use std::ptr::null_mut;
use std::thread;
use std::time::Duration;

use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmGetOpenStatus, ImmReleaseContext, ImmSetOpenStatus,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, LoadKeyboardLayoutW, KLF_ACTIVATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

use crate::models::{FocusedApp, LanguageMode};

use super::{PlatformCapabilities, PlatformController};

const PRIMARY_LANG_CHINESE: u16 = 0x04;
#[derive(Default)]
pub struct WindowsPlatformController;

impl WindowsPlatformController {
    fn foreground_window(&self) -> Result<HWND, String> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0 == null_mut() {
            Err("Unable to locate a foreground window.".to_string())
        } else {
            Ok(hwnd)
        }
    }

    fn read_window_title(&self, hwnd: HWND) -> String {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return String::new();
        }

        let mut buffer = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..copied as usize])
    }

    fn process_path(&self, pid: u32) -> Result<Option<String>, String> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|err| format!("Unable to open process {pid}: {err}"))?;

        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;
        let query_result = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        };
        let _ = unsafe { CloseHandle(handle) };

        query_result.map_err(|err| format!("Unable to read process path for {pid}: {err}"))?;

        Ok(Some(String::from_utf16_lossy(&buffer[..size as usize])))
    }

    fn language_from_hwnd(&self, hwnd: HWND) -> Result<Option<LanguageMode>, String> {
        let thread_id = unsafe { GetWindowThreadProcessId(hwnd, None) };
        if thread_id == 0 {
            return Err("Unable to resolve the input thread for the active window.".to_string());
        }

        let layout = unsafe { GetKeyboardLayout(thread_id) };
        let lang_id = ((layout.0 as usize) & 0xffff) as u16;
        let primary_lang = lang_id & 0x03ff;

        let himc = unsafe { ImmGetContext(hwnd) };
        let ime_open = if himc.0 != null_mut() {
            let is_open = unsafe { ImmGetOpenStatus(himc).as_bool() };
            let _ = unsafe { ImmReleaseContext(hwnd, himc) };
            is_open
        } else {
            false
        };

        if primary_lang == PRIMARY_LANG_CHINESE && ime_open {
            Ok(Some(LanguageMode::Chinese))
        } else {
            Ok(Some(LanguageMode::English))
        }
    }

    fn wait_for_language(&self, hwnd: HWND, target: LanguageMode) -> Result<bool, String> {
        for _ in 0..6 {
            if self.language_from_hwnd(hwnd)? == Some(target) {
                return Ok(true);
            }

            thread::sleep(Duration::from_millis(40));
        }

        Ok(false)
    }

    fn change_language(&self, hwnd: HWND, target: LanguageMode) -> Result<bool, String> {
        match target {
            LanguageMode::Chinese => {
                let hkl = unsafe { LoadKeyboardLayoutW(w!("00000804"), KLF_ACTIVATE) }
                    .map_err(|err| format!("Unable to load Chinese keyboard layout: {err}"))?;
                let post_message_sent = unsafe {
                    PostMessageW(
                        hwnd,
                        WM_INPUTLANGCHANGEREQUEST,
                        WPARAM(0),
                        LPARAM(hkl.0 as isize),
                    )
                }
                .is_ok();

                thread::sleep(Duration::from_millis(80));

                let himc = unsafe { ImmGetContext(hwnd) };
                let mut ime_opened = false;
                if himc.0 != null_mut() {
                    ime_opened = unsafe { ImmSetOpenStatus(himc, true) }.as_bool();
                    let _ = unsafe { ImmReleaseContext(hwnd, himc) };
                }

                if !post_message_sent && !ime_opened {
                    return Ok(false);
                }
            }
            LanguageMode::English => {
                let himc = unsafe { ImmGetContext(hwnd) };
                let mut ime_closed = false;
                if himc.0 != null_mut() {
                    ime_closed = unsafe { ImmSetOpenStatus(himc, false) }.as_bool();
                    let _ = unsafe { ImmReleaseContext(hwnd, himc) };
                }

                let hkl = unsafe { LoadKeyboardLayoutW(w!("00000409"), KLF_ACTIVATE) }
                    .map_err(|err| format!("Unable to load English keyboard layout: {err}"))?;
                let post_message_sent = unsafe {
                    PostMessageW(
                        hwnd,
                        WM_INPUTLANGCHANGEREQUEST,
                        WPARAM(0),
                        LPARAM(hkl.0 as isize),
                    )
                }
                .is_ok();

                if !post_message_sent && !ime_closed {
                    return Ok(false);
                }
            }
        }

        self.wait_for_language(hwnd, target)
    }
}

impl PlatformController for WindowsPlatformController {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            name: "windows",
            supports_focus_tracking: true,
            supports_input_control: true,
            notes: "Uses the foreground window, process executable, and IME open status on Windows. Chinese and English switching is triggered only on application focus changes.",
        }
    }

    fn active_application(&self) -> Result<Option<FocusedApp>, String> {
        let hwnd = self.foreground_window()?;
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };

        if pid == 0 {
            return Ok(None);
        }

        let process_path = self.process_path(pid)?;
        let executable = process_path
            .as_deref()
            .and_then(|path| Path::new(path).file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("pid-{pid}"));

        Ok(Some(FocusedApp {
            executable,
            process_path,
            window_title: self.read_window_title(hwnd),
            pid,
        }))
    }

    fn current_input_mode(&self, _app: &FocusedApp) -> Result<Option<LanguageMode>, String> {
        let hwnd = self.foreground_window()?;
        self.language_from_hwnd(hwnd)
    }

    fn set_input_mode(&self, _app: &FocusedApp, mode: LanguageMode) -> Result<bool, String> {
        let hwnd = self.foreground_window()?;
        self.change_language(hwnd, mode)
    }
}
