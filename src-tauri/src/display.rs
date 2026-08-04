use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayMode {
    pub display_id: String,
    pub device_name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayProfile {
    pub game_id: String,
    pub enabled: bool,
    pub display_id: Option<String>,
    pub device_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh_rate: Option<u32>,
    pub restore_on_exit: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingDisplayRestore {
    pub display_id: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub created_at: String,
}

#[cfg(windows)]
mod windows_impl {
    use super::{DisplayMode, PendingDisplayRestore};
    use std::{mem, ptr};
    use windows_sys::Win32::Graphics::Gdi::{
        ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsExW, CDS_TEST, DEVMODEW,
        DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_PRIMARY_DEVICE,
        DISP_CHANGE_SUCCESSFUL, ENUM_CURRENT_SETTINGS,
    };

    pub fn primary_display_id() -> Result<String, String> {
        unsafe {
            let mut index = 0;
            let mut fallback = None;
            loop {
                let mut device: DISPLAY_DEVICEW = mem::zeroed();
                device.cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;
                if EnumDisplayDevicesW(ptr::null(), index, &mut device, 0) == 0 {
                    break;
                }
                let name = wide_string(&device.DeviceName);
                if name.is_empty() {
                    index += 1;
                    continue;
                }
                if device.StateFlags & DISPLAY_DEVICE_ACTIVE != 0 {
                    fallback = Some(name.clone());
                }
                if device.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0 {
                    return Ok(name);
                }
                index += 1;
            }
            fallback
                .or_else(|| Some(r"\\.\DISPLAY1".to_string()))
                .ok_or_else(|| "DISPLAY_NOT_FOUND".to_string())
        }
    }

    pub fn current_mode(display_id: Option<&str>) -> Result<DisplayMode, String> {
        let display_id = display_id
            .map(ToOwned::to_owned)
            .unwrap_or(primary_display_id()?);
        let mode = enum_mode(&display_id, ENUM_CURRENT_SETTINGS)
            .ok_or_else(|| "DISPLAY_CURRENT_MODE_UNAVAILABLE".to_string())?;
        Ok(mode_to_public(&display_id, &mode))
    }

    pub fn enumerate_modes() -> Result<Vec<DisplayMode>, String> {
        let display_id = primary_display_id()?;
        let mut modes = Vec::new();
        let mut index = 0;
        while let Some(mode) = enum_mode(&display_id, index) {
            if mode.dmPelsWidth > 0 && mode.dmPelsHeight > 0 && mode.dmDisplayFrequency > 0 {
                let public = mode_to_public(&display_id, &mode);
                if !modes.iter().any(|item: &DisplayMode| item == &public) {
                    modes.push(public);
                }
            }
            index += 1;
        }
        modes.sort_by_key(|mode| (mode.width, mode.height, mode.refresh_rate));
        if modes.is_empty() {
            return Err("DISPLAY_MODES_UNAVAILABLE".to_string());
        }
        Ok(modes)
    }

    pub fn apply_mode(request: &DisplayMode) -> Result<DisplayMode, String> {
        let display_id = if request.display_id.is_empty() {
            primary_display_id()?
        } else {
            request.display_id.clone()
        };
        let mut index = 0;
        let mut selected = None;
        while let Some(mode) = enum_mode(&display_id, index) {
            if mode.dmPelsWidth == request.width
                && mode.dmPelsHeight == request.height
                && mode.dmDisplayFrequency == request.refresh_rate
            {
                selected = Some(mode);
                break;
            }
            index += 1;
        }
        let mode = selected.ok_or_else(|| "DISPLAY_MODE_UNAVAILABLE".to_string())?;
        let device_name = wide_string(&mode.dmDeviceName);
        let display_name = to_wide(&display_id);
        let test_result = unsafe {
            ChangeDisplaySettingsExW(
                display_name.as_ptr(),
                &mode,
                ptr::null_mut(),
                CDS_TEST,
                ptr::null(),
            )
        };
        if test_result != DISP_CHANGE_SUCCESSFUL {
            return Err(format!("DISPLAY_MODE_TEST_REJECTED:{test_result}"));
        }
        let apply_result = unsafe {
            ChangeDisplaySettingsExW(
                display_name.as_ptr(),
                &mode,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };
        if apply_result != DISP_CHANGE_SUCCESSFUL {
            return Err(format!("DISPLAY_MODE_APPLY_FAILED:{apply_result}"));
        }
        let applied = current_mode(Some(&display_id))?;
        if applied.width != request.width
            || applied.height != request.height
            || applied.refresh_rate != request.refresh_rate
        {
            return Err("DISPLAY_MODE_VERIFY_FAILED".to_string());
        }
        Ok(DisplayMode {
            display_id,
            device_name,
            width: request.width,
            height: request.height,
            refresh_rate: request.refresh_rate,
        })
    }

    pub fn restore_mode(pending: &PendingDisplayRestore) -> Result<DisplayMode, String> {
        apply_mode(&DisplayMode {
            display_id: pending.display_id.clone(),
            device_name: pending.display_id.clone(),
            width: pending.width,
            height: pending.height,
            refresh_rate: pending.refresh_rate,
        })
    }

    fn enum_mode(display_id: &str, mode_index: u32) -> Option<DEVMODEW> {
        let display_name = to_wide(display_id);
        unsafe {
            let mut mode: DEVMODEW = mem::zeroed();
            mode.dmSize = mem::size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsExW(display_name.as_ptr(), mode_index, &mut mode, 0) == 0 {
                None
            } else {
                Some(mode)
            }
        }
    }

    fn mode_to_public(display_id: &str, mode: &DEVMODEW) -> DisplayMode {
        DisplayMode {
            display_id: display_id.to_string(),
            device_name: wide_string(&mode.dmDeviceName),
            width: mode.dmPelsWidth,
            height: mode.dmPelsHeight,
            refresh_rate: mode.dmDisplayFrequency,
        }
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn wide_string(value: &[u16]) -> String {
        let end = value
            .iter()
            .position(|item| *item == 0)
            .unwrap_or(value.len());
        String::from_utf16_lossy(&value[..end])
    }
}

#[cfg(windows)]
pub use windows_impl::{
    apply_mode, current_mode, enumerate_modes, primary_display_id, restore_mode,
};

#[cfg(not(windows))]
pub fn primary_display_id() -> Result<String, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn current_mode(_display_id: Option<&str>) -> Result<DisplayMode, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn enumerate_modes() -> Result<Vec<DisplayMode>, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn apply_mode(_request: &DisplayMode) -> Result<DisplayMode, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn restore_mode(_pending: &PendingDisplayRestore) -> Result<DisplayMode, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}
