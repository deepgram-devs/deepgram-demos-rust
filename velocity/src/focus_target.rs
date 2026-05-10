use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow, SetForegroundWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTarget(isize);

impl FocusTarget {
    pub fn current() -> Option<Self> {
        let hwnd = unsafe { GetForegroundWindow() };
        Self::from_hwnd(hwnd)
    }

    pub fn from_raw(raw: isize) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    pub fn raw(self) -> isize {
        self.0
    }

    pub fn focus(self) -> Result<(), String> {
        let hwnd = self.hwnd();
        if hwnd.0.is_null() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err("The original target window is no longer available".to_string());
        }

        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            return Err("Failed to focus the original target window".to_string());
        }

        Ok(())
    }

    fn from_hwnd(hwnd: HWND) -> Option<Self> {
        (!hwnd.0.is_null()).then_some(Self(hwnd.0 as isize))
    }

    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}
