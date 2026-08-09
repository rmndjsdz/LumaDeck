use crate::rtx_hdr::RtxHdrPreset;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
pub struct DisplayModeDiagnostics {
    pub display_id: String,
    pub device_name: String,
    pub mode_index: Option<u32>,
    pub dm_size: u16,
    pub dm_driver_extra: u16,
    pub width: u32,
    pub height: u32,
    pub bits_per_pixel: u32,
    pub frequency: u32,
    pub orientation: u32,
    pub dm_fields: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayModeTestResult {
    pub display_id: String,
    pub current: DisplayModeDiagnostics,
    pub target: DisplayModeDiagnostics,
    pub test_result: i32,
    pub accepted: bool,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayScale {
    pub current: Option<u32>,
    pub recommended: Option<u32>,
    pub supported: Vec<u32>,
    pub available: bool,
    pub can_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HdrStatus {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HdrState {
    pub display_id: String,
    pub supported: Option<bool>,
    pub enabled: Option<bool>,
    pub status: HdrStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HdrSnapshot {
    pub display_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySnapshot {
    pub display_id: String,
    pub mode: Option<DisplayMode>,
    pub scale: DisplayScale,
    pub hdr: HdrSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub friendly_name: Option<String>,
    pub primary: bool,
    pub connected: bool,
    pub current_mode: Option<DisplayMode>,
    pub scale: DisplayScale,
    pub hdr_supported: Option<bool>,
    pub hdr_enabled: Option<bool>,
    pub hdr_status: HdrStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayModeChange {
    pub previous_mode: DisplayMode,
    pub applied_mode: DisplayMode,
    pub expires_at_ms: u64,
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
    #[serde(default)]
    pub resolution_mode: DisplayResolutionMode,
    #[serde(default)]
    pub refresh_rate_mode: DisplayRefreshRateMode,
    #[serde(default)]
    pub hdr_mode: DisplayHdrMode,
    #[serde(default)]
    pub rtx_hdr_preset: Option<RtxHdrPreset>,
    #[serde(default = "default_rtx_hdr_peak_nits")]
    pub rtx_hdr_peak_nits: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisplayResolutionMode {
    System,
    Custom,
}

impl Default for DisplayResolutionMode {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisplayRefreshRateMode {
    System,
    Custom,
}

impl Default for DisplayRefreshRateMode {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisplayHdrMode {
    System,
    Off,
    On,
    Auto,
}

impl Default for DisplayHdrMode {
    fn default() -> Self {
        Self::System
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayProfileSnapshot {
    pub display_id: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub hdr_enabled: bool,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingDisplayProfileRestore {
    pub session_id: String,
    pub game_id: String,
    pub snapshot: DisplayProfileSnapshot,
    pub changed_resolution: bool,
    pub changed_refresh_rate: bool,
    pub changed_hdr: bool,
    pub rtx_hdr_snapshot: Option<String>,
    pub auto_hdr_snapshot: Option<String>,
    pub rtx_hdr_executable: Option<String>,
    pub changed_rtx_hdr: bool,
    pub changed_auto_hdr: bool,
}

fn default_rtx_hdr_peak_nits() -> u32 {
    crate::rtx_hdr::RTX_HDR_PEAK_NITS_DEFAULT
}

#[derive(Clone, Default)]
pub struct DisplayConfirmationService {
    active: Arc<Mutex<Option<ActiveConfirmation>>>,
}

#[derive(Debug, Clone)]
struct ActiveConfirmation {
    previous_mode: DisplayMode,
    token: u64,
}

impl DisplayConfirmationService {
    pub fn has_pending(&self) -> bool {
        self.active.lock().ok().is_some_and(|guard| guard.is_some())
    }

    pub fn arm<F>(
        &self,
        previous_mode: DisplayMode,
        pending_path: std::path::PathBuf,
        timeout: Duration,
        on_restored: F,
    ) where
        F: Fn() + Send + Sync + 'static,
    {
        let token = now_millis();
        if let Ok(mut active) = self.active.lock() {
            *active = Some(ActiveConfirmation {
                previous_mode: previous_mode.clone(),
                token,
            });
        }
        let active = Arc::clone(&self.active);
        let on_restored = Arc::new(on_restored);
        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            let should_restore = active
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
                .is_some_and(|confirmation| confirmation.token == token);
            if !should_restore {
                return;
            }
            let restored = restore_mode(&PendingDisplayRestore {
                display_id: previous_mode.display_id.clone(),
                width: previous_mode.width,
                height: previous_mode.height,
                refresh_rate: previous_mode.refresh_rate,
                created_at: String::new(),
            });
            if restored.is_ok() {
                on_restored();
                if let Ok(mut guard) = active.lock() {
                    if guard.as_ref().is_some_and(|item| item.token == token) {
                        *guard = None;
                    }
                }
                clear_pending_restore_file(&pending_path);
            }
        });
    }

    pub fn confirm(&self) {
        if let Ok(mut active) = self.active.lock() {
            *active = None;
        }
    }

    pub fn take_previous(&self) -> Option<DisplayMode> {
        self.active
            .lock()
            .ok()
            .and_then(|mut guard| guard.take().map(|item| item.previous_mode))
    }
}

pub fn now_timestamp() -> String {
    now_millis().to_string()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn clear_pending_restore_file(path: &std::path::Path) {
    if let Ok(connection) = rusqlite::Connection::open(path) {
        let _ = connection.execute("DELETE FROM pending_display_restore WHERE id = 1", []);
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        DisplayInfo, DisplayMode, DisplayModeDiagnostics, DisplayModeTestResult, DisplayScale,
        DisplaySnapshot, HdrSnapshot, HdrState, HdrStatus, PendingDisplayRestore,
    };
    use std::{mem, ptr, thread, time::Duration};
    use windows_sys::Win32::{
        Devices::Display::{
            DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
            QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_PATH_INFO,
            DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
        },
        Graphics::Gdi::{
            EnumDisplayDevicesW, EnumDisplaySettingsExW, DEVMODEW, DISPLAY_DEVICEW,
            DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_PRIMARY_DEVICE, DISP_CHANGE_FAILED,
            DISP_CHANGE_SUCCESSFUL, ENUM_CURRENT_SETTINGS,
        },
    };

    const DISPLAYCONFIG_DEVICE_INFO_GET_DPI_SCALE: i32 = -3;
    const DISPLAYCONFIG_DEVICE_INFO_SET_DPI_SCALE: i32 = -4;
    const DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO: i32 = 9;
    const DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE: i32 = 10;
    const HDR_RECONCILIATION_ATTEMPTS: u8 = 20;
    const HDR_RECONCILIATION_INTERVAL: Duration = Duration::from_millis(100);
    const DPI_SCALE_VALUES: [u32; 12] =
        [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct DisplayConfigSourceDpiScaleGet {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        min_scale_relative: i32,
        current_scale_relative: i32,
        max_scale_relative: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct DisplayConfigSourceDpiScaleSet {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        scale_relative: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct DisplayConfigGetAdvancedColorInfo {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        value: u32,
        color_encoding: i32,
        bits_per_color_channel: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct DisplayConfigSetAdvancedColorState {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        value: u32,
    }

    #[derive(Clone, Copy)]
    struct DisplayConfigPath {
        source_adapter_id: windows_sys::Win32::Foundation::LUID,
        source_id: u32,
        target_adapter_id: windows_sys::Win32::Foundation::LUID,
        target_id: u32,
    }

    pub fn enumerate_displays() -> Result<Vec<DisplayInfo>, String> {
        let devices = display_devices();
        if devices.is_empty() {
            return Err("DISPLAY_NOT_FOUND".to_string());
        }
        let mut displays = Vec::with_capacity(devices.len());
        for (id, friendly_name, state_flags) in devices {
            if state_flags & DISPLAY_DEVICE_ACTIVE == 0 {
                continue;
            }
            let current_mode = current_mode(Some(&id)).ok();
            let scale = get_display_scale(&id).unwrap_or_else(|_| DisplayScale {
                current: None,
                recommended: None,
                supported: Vec::new(),
                available: false,
                can_change: false,
            });
            let hdr = get_hdr_state(&id).unwrap_or_else(|_| HdrState {
                display_id: id.clone(),
                supported: None,
                enabled: None,
                status: HdrStatus::Unknown,
            });
            displays.push(DisplayInfo {
                name: friendly_name.clone(),
                friendly_name: (!friendly_name.is_empty()).then_some(friendly_name),
                primary: state_flags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0,
                connected: current_mode.is_some(),
                current_mode,
                scale,
                hdr_supported: hdr.supported,
                hdr_enabled: hdr.enabled,
                hdr_status: hdr.status,
                id,
            });
        }
        displays.sort_by_key(|display| (!display.primary, display.id.clone()));
        Ok(displays)
    }

    pub fn primary_display_id() -> Result<String, String> {
        let devices = display_devices();
        devices
            .iter()
            .find(|(_, _, state_flags)| state_flags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0)
            .or_else(|| {
                devices
                    .iter()
                    .find(|(_, _, state_flags)| state_flags & DISPLAY_DEVICE_ACTIVE != 0)
            })
            .map(|(id, _, _)| id.clone())
            .ok_or_else(|| "DISPLAY_NOT_FOUND".to_string())
    }

    pub fn current_mode(display_id: Option<&str>) -> Result<DisplayMode, String> {
        let display_id = match display_id {
            Some(display_id) => display_id.to_string(),
            None => primary_display_id()?,
        };
        let mode = enum_mode(&display_id, ENUM_CURRENT_SETTINGS)
            .ok_or_else(|| "DISPLAY_CURRENT_MODE_UNAVAILABLE".to_string())?;
        Ok(mode_to_public(&display_id, &mode))
    }

    pub fn enumerate_modes() -> Result<Vec<DisplayMode>, String> {
        enumerate_modes_for_display(&primary_display_id()?)
    }

    pub fn enumerate_modes_for_display(display_id: &str) -> Result<Vec<DisplayMode>, String> {
        let mut modes = Vec::new();
        let mut index = 0;
        while let Some(mode) = enum_mode(display_id, index) {
            if mode.dmPelsWidth > 0 && mode.dmPelsHeight > 0 && mode.dmDisplayFrequency > 0 {
                let public = mode_to_public(display_id, &mode);
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

    pub fn test_current_display_mode(
        display_id: Option<&str>,
    ) -> Result<DisplayModeTestResult, String> {
        let display_id = display_id
            .map(str::to_string)
            .unwrap_or(primary_display_id()?);
        let current = enum_mode(&display_id, ENUM_CURRENT_SETTINGS)
            .ok_or_else(|| "DISPLAY_CURRENT_MODE_UNAVAILABLE".to_string())?;
        let target = current;
        let display_name = to_wide(&display_id);
        let test_result = unsafe {
            windows_sys::Win32::Graphics::Gdi::ChangeDisplaySettingsExW(
                display_name.as_ptr(),
                &target,
                ptr::null_mut(),
                windows_sys::Win32::Graphics::Gdi::CDS_TEST,
                ptr::null(),
            )
        };
        Ok(DisplayModeTestResult {
            display_id: display_id.clone(),
            current: mode_to_diagnostics(&display_id, None, &current),
            target: mode_to_diagnostics(&display_id, None, &target),
            test_result,
            accepted: test_result == DISP_CHANGE_SUCCESSFUL,
            diagnostic: (test_result == DISP_CHANGE_FAILED)
                .then_some("DISPLAY_API_TEST_UNAVAILABLE".to_string()),
        })
    }

    pub fn apply_mode(request: &DisplayMode) -> Result<DisplayMode, String> {
        let display_id = if request.display_id.is_empty() {
            primary_display_id()?
        } else {
            request.display_id.clone()
        };
        let current = current_mode(Some(&display_id))?;
        let (mode, mode_index, native_mode) = find_native_mode(&display_id, request)?;
        if current.width == mode.width
            && current.height == mode.height
            && current.refresh_rate == mode.refresh_rate
        {
            return Ok(current);
        }
        let display_name = to_wide(&display_id);
        let test_result = unsafe {
            windows_sys::Win32::Graphics::Gdi::ChangeDisplaySettingsExW(
                display_name.as_ptr(),
                &native_mode,
                ptr::null_mut(),
                windows_sys::Win32::Graphics::Gdi::CDS_TEST,
                ptr::null(),
            )
        };
        if test_result != DISP_CHANGE_SUCCESSFUL {
            let diagnostics = mode_to_diagnostics(&display_id, Some(mode_index), &native_mode);
            let current_native = enum_mode(&display_id, ENUM_CURRENT_SETTINGS)
                .ok_or_else(|| "DISPLAY_CURRENT_MODE_UNAVAILABLE".to_string())?;
            let current_diagnostics = mode_to_diagnostics(&display_id, None, &current_native);
            if test_result != DISP_CHANGE_FAILED {
                return Err(format!(
                    "DISPLAY_MODE_TEST_REJECTED:{test_result};current={};target={}",
                    diagnostics_string(&current_diagnostics),
                    diagnostics_string(&diagnostics)
                ));
            }
            let fallback_result = unsafe {
                windows_sys::Win32::Graphics::Gdi::ChangeDisplaySettingsExW(
                    display_name.as_ptr(),
                    &native_mode,
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                )
            };
            if fallback_result != DISP_CHANGE_SUCCESSFUL {
                return Err(format!(
                    "DISPLAY_MODE_TEST_REJECTED:{test_result};DISPLAY_MODE_FALLBACK_FAILED:{fallback_result};current={};target={}",
                    diagnostics_string(&current_diagnostics),
                    diagnostics_string(&diagnostics)
                ));
            }
        } else {
            let apply_result = unsafe {
                windows_sys::Win32::Graphics::Gdi::ChangeDisplaySettingsExW(
                    display_name.as_ptr(),
                    &native_mode,
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                )
            };
            if apply_result != DISP_CHANGE_SUCCESSFUL {
                return Err(format!("DISPLAY_MODE_APPLY_FAILED:{apply_result}"));
            }
        }
        let applied = current_mode(Some(&display_id))?;
        if applied.width != mode.width
            || applied.height != mode.height
            || applied.refresh_rate != mode.refresh_rate
        {
            return Err(format!(
                "DISPLAY_MODE_VERIFY_FAILED;target={}",
                diagnostics_string(&mode_to_diagnostics(
                    &display_id,
                    Some(mode_index),
                    &native_mode
                ))
            ));
        }
        Ok(applied)
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

    pub fn get_display_scale(display_id: &str) -> Result<DisplayScale, String> {
        let (adapter_id, source_id) = display_config_source(display_id)?;
        display_scale_for_source(adapter_id, source_id)
    }

    pub fn set_display_scale(display_id: &str, scale: u32) -> Result<DisplayScale, String> {
        let (adapter_id, source_id) = display_config_source(display_id)?;
        let current = display_scale_for_source(adapter_id, source_id)?;
        if !current.supported.contains(&scale) {
            return Err("DISPLAY_SCALE_UNAVAILABLE".to_string());
        }
        if current.current == Some(scale) {
            return Ok(current);
        }
        let recommended_index = current
            .supported
            .iter()
            .position(|value| Some(*value) == current.recommended)
            .ok_or_else(|| "DISPLAY_SCALE_UNAVAILABLE".to_string())?;
        let target_index = current
            .supported
            .iter()
            .position(|value| *value == scale)
            .ok_or_else(|| "DISPLAY_SCALE_UNAVAILABLE".to_string())?;
        let relative = i32::try_from(target_index)
            .ok()
            .and_then(|target| {
                i32::try_from(recommended_index)
                    .ok()
                    .map(|recommended| target - recommended)
            })
            .ok_or_else(|| "DISPLAY_SCALE_UNAVAILABLE".to_string())?;
        let request = DisplayConfigSourceDpiScaleSet {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_SET_DPI_SCALE,
                size: mem::size_of::<DisplayConfigSourceDpiScaleSet>() as u32,
                adapterId: adapter_id,
                id: source_id,
            },
            scale_relative: relative,
        };
        let result = unsafe { DisplayConfigSetDeviceInfo(&request.header) };
        if result != 0 {
            return Err(format!("DISPLAY_SCALE_APPLY_FAILED:{result}"));
        }
        display_scale_for_source(adapter_id, source_id)
    }

    pub fn get_hdr_state(display_id: &str) -> Result<HdrState, String> {
        let path = display_config_path(display_id)?;
        let mut request = DisplayConfigGetAdvancedColorInfo {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                size: mem::size_of::<DisplayConfigGetAdvancedColorInfo>() as u32,
                adapterId: path.target_adapter_id,
                id: path.target_id,
            },
            value: 0,
            color_encoding: 0,
            bits_per_color_channel: 0,
        };
        let result = unsafe { DisplayConfigGetDeviceInfo(&mut request.header) };
        if result != 0 {
            return Err(format!("DISPLAY_HDR_STATE:{result}"));
        }
        let supported = request.value & 1 != 0;
        Ok(HdrState {
            display_id: display_id.to_string(),
            supported: Some(supported),
            enabled: Some(supported && request.value & 2 != 0),
            status: if supported {
                HdrStatus::Supported
            } else {
                HdrStatus::Unsupported
            },
        })
    }

    pub fn set_hdr_enabled(display_id: &str, enabled: bool) -> Result<HdrState, String> {
        let current = get_hdr_state(display_id)?;
        if current.status != HdrStatus::Supported || current.supported != Some(true) {
            return Err("DISPLAY_HDR_UNSUPPORTED".to_string());
        }
        if current.enabled == Some(enabled) {
            return Ok(current);
        }
        let path = display_config_path(display_id)?;
        let request = DisplayConfigSetAdvancedColorState {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
                size: mem::size_of::<DisplayConfigSetAdvancedColorState>() as u32,
                adapterId: path.target_adapter_id,
                id: path.target_id,
            },
            value: u32::from(enabled),
        };
        let result = unsafe { DisplayConfigSetDeviceInfo(&request.header) };
        if result != 0 {
            return Err(format!("DISPLAY_HDR_APPLY_FAILED:{result}"));
        }

        for attempt in 0..=HDR_RECONCILIATION_ATTEMPTS {
            match get_hdr_state(display_id) {
                Ok(state) if state.enabled == Some(enabled) => return Ok(state),
                Ok(_) if attempt < HDR_RECONCILIATION_ATTEMPTS => {
                    thread::sleep(HDR_RECONCILIATION_INTERVAL);
                }
                Ok(_) => break,
                Err(error) => return Err(error),
            }
        }
        Err("DISPLAY_HDR_VERIFY_FAILED".to_string())
    }

    pub fn capture_hdr_state(display_id: &str) -> Result<HdrSnapshot, String> {
        let state = get_hdr_state(display_id)?;
        let Some(enabled) = state.enabled else {
            return Err("DISPLAY_HDR_UNAVAILABLE".to_string());
        };
        Ok(HdrSnapshot {
            display_id: display_id.to_string(),
            enabled,
        })
    }

    pub fn restore_hdr_state(snapshot: &HdrSnapshot) -> Result<HdrState, String> {
        let current = get_hdr_state(&snapshot.display_id)?;
        if current.status == HdrStatus::Unsupported && !snapshot.enabled {
            return Ok(current);
        }
        set_hdr_enabled(&snapshot.display_id, snapshot.enabled)
    }

    pub fn capture_display_snapshot(display_id: &str) -> Result<DisplaySnapshot, String> {
        Ok(DisplaySnapshot {
            display_id: display_id.to_string(),
            mode: current_mode(Some(display_id)).ok(),
            scale: get_display_scale(display_id)?,
            hdr: capture_hdr_state(display_id)?,
        })
    }

    pub fn restore_display_snapshot(snapshot: &DisplaySnapshot) -> Result<DisplaySnapshot, String> {
        if let Some(mode) = &snapshot.mode {
            let current = current_mode(Some(&snapshot.display_id)).ok();
            if current.as_ref() != Some(mode) {
                apply_mode(mode)?;
            }
        }
        if let Some(scale) = snapshot.scale.current {
            let current = get_display_scale(&snapshot.display_id)?;
            if current.current != Some(scale) {
                set_display_scale(&snapshot.display_id, scale)?;
            }
        }
        restore_hdr_state(&snapshot.hdr)?;
        capture_display_snapshot(&snapshot.display_id)
    }

    fn display_scale_for_source(
        adapter_id: windows_sys::Win32::Foundation::LUID,
        source_id: u32,
    ) -> Result<DisplayScale, String> {
        let mut request = DisplayConfigSourceDpiScaleGet {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_DPI_SCALE,
                size: mem::size_of::<DisplayConfigSourceDpiScaleGet>() as u32,
                adapterId: adapter_id,
                id: source_id,
            },
            min_scale_relative: 0,
            current_scale_relative: 0,
            max_scale_relative: 0,
        };
        let result = unsafe { DisplayConfigGetDeviceInfo(&mut request.header) };
        if result != 0 {
            return Err(format!("DISPLAY_SCALE_UNAVAILABLE:{result}"));
        }
        let (min_index, current_index, max_index, recommended_index) = scale_indices(
            request.min_scale_relative,
            request.current_scale_relative,
            request.max_scale_relative,
        )?;
        if min_index > max_index
            || max_index >= DPI_SCALE_VALUES.len()
            || current_index >= DPI_SCALE_VALUES.len()
            || recommended_index >= DPI_SCALE_VALUES.len()
        {
            return Err("DISPLAY_SCALE_VALUES_UNSUPPORTED".to_string());
        }
        let supported = DPI_SCALE_VALUES[min_index..=max_index].to_vec();
        Ok(DisplayScale {
            current: Some(DPI_SCALE_VALUES[current_index]),
            recommended: Some(DPI_SCALE_VALUES[recommended_index]),
            can_change: supported.len() > 1,
            supported,
            available: true,
        })
    }

    fn scale_indices(
        min_scale_relative: i32,
        current_scale_relative: i32,
        max_scale_relative: i32,
    ) -> Result<(usize, usize, usize, usize), String> {
        let recommended_index = min_scale_relative
            .checked_neg()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "DISPLAY_SCALE_VALUES_UNSUPPORTED".to_string())?;
        let to_index = |relative: i32| {
            i64::try_from(recommended_index)
                .ok()
                .and_then(|value| value.checked_add(i64::from(relative)))
                .and_then(|value| usize::try_from(value).ok())
        };
        let min_index = to_index(min_scale_relative)
            .ok_or_else(|| "DISPLAY_SCALE_VALUES_UNSUPPORTED".to_string())?;
        let current_index = to_index(current_scale_relative)
            .ok_or_else(|| "DISPLAY_SCALE_VALUES_UNSUPPORTED".to_string())?;
        let max_index = to_index(max_scale_relative)
            .ok_or_else(|| "DISPLAY_SCALE_VALUES_UNSUPPORTED".to_string())?;
        Ok((min_index, current_index, max_index, recommended_index))
    }

    fn display_config_source(
        display_id: &str,
    ) -> Result<(windows_sys::Win32::Foundation::LUID, u32), String> {
        let path = display_config_path(display_id)?;
        Ok((path.source_adapter_id, path.source_id))
    }

    fn display_config_path(display_id: &str) -> Result<DisplayConfigPath, String> {
        let mut path_count = 0;
        let mut mode_count = 0;
        let result = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if result != 0 {
            return Err(format!("DISPLAY_SCALE_UNAVAILABLE:{result}"));
        }
        let mut paths =
            vec![unsafe { mem::zeroed::<DISPLAYCONFIG_PATH_INFO>() }; path_count as usize];
        let mut modes = vec![unsafe { mem::zeroed() }; mode_count as usize];
        let result = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                ptr::null_mut(),
            )
        };
        if result != 0 {
            return Err(format!("DISPLAY_SCALE_UNAVAILABLE:{result}"));
        }
        for path in paths.iter().take(path_count as usize) {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                viewGdiDeviceName: [0; 32],
            };
            let result = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) };
            if result == 0 && wide_string(&source.viewGdiDeviceName) == display_id {
                return Ok(DisplayConfigPath {
                    source_adapter_id: path.sourceInfo.adapterId,
                    source_id: path.sourceInfo.id,
                    target_adapter_id: path.targetInfo.adapterId,
                    target_id: path.targetInfo.id,
                });
            }
        }
        Err("DISPLAY_SCALE_UNAVAILABLE".to_string())
    }

    fn display_devices() -> Vec<(String, String, u32)> {
        let mut devices = Vec::new();
        let mut index = 0;
        loop {
            let mut device: DISPLAY_DEVICEW = unsafe { mem::zeroed() };
            device.cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;
            let result = unsafe { EnumDisplayDevicesW(ptr::null(), index, &mut device, 0) };
            if result == 0 {
                break;
            }
            let id = wide_string(&device.DeviceName);
            if !id.is_empty() {
                devices.push((id, wide_string(&device.DeviceString), device.StateFlags));
            }
            index += 1;
        }
        devices
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

    fn find_native_mode(
        display_id: &str,
        wanted: &DisplayMode,
    ) -> Result<(DisplayMode, u32, DEVMODEW), String> {
        let mut index = 0;
        while let Some(mode) = enum_mode(display_id, index) {
            if mode.dmPelsWidth == wanted.width
                && mode.dmPelsHeight == wanted.height
                && mode.dmDisplayFrequency == wanted.refresh_rate
            {
                return Ok((mode_to_public(display_id, &mode), index, mode));
            }
            index += 1;
        }
        Err("DISPLAY_MODE_UNAVAILABLE".to_string())
    }

    fn mode_to_diagnostics(
        display_id: &str,
        mode_index: Option<u32>,
        mode: &DEVMODEW,
    ) -> DisplayModeDiagnostics {
        DisplayModeDiagnostics {
            display_id: display_id.to_string(),
            device_name: wide_string(&mode.dmDeviceName),
            mode_index,
            dm_size: mode.dmSize,
            dm_driver_extra: mode.dmDriverExtra,
            width: mode.dmPelsWidth,
            height: mode.dmPelsHeight,
            bits_per_pixel: mode.dmBitsPerPel,
            frequency: mode.dmDisplayFrequency,
            orientation: unsafe { mode.Anonymous1.Anonymous2.dmDisplayOrientation },
            dm_fields: mode.dmFields,
        }
    }

    fn diagnostics_string(value: &DisplayModeDiagnostics) -> String {
        format!(
            "deviceName={};index={:?};dmSize={};dmDriverExtra={};width={};height={};bitsPerPel={};frequency={};orientation={};dmFields=0x{:08x}",
            value.device_name,
            value.mode_index,
            value.dm_size,
            value.dm_driver_extra,
            value.width,
            value.height,
            value.bits_per_pixel,
            value.frequency,
            value.orientation,
            value.dm_fields
        )
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

    #[cfg(test)]
    mod tests {
        use super::scale_indices;

        #[test]
        fn scale_range_starts_below_recommended_value() {
            assert_eq!(scale_indices(-4, -4, 1).unwrap(), (0, 0, 5, 4));
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{
    apply_mode, capture_display_snapshot, capture_hdr_state, current_mode, enumerate_displays,
    enumerate_modes, enumerate_modes_for_display, get_display_scale, get_hdr_state,
    primary_display_id, restore_display_snapshot, restore_hdr_state, restore_mode,
    set_display_scale, set_hdr_enabled, test_current_display_mode,
};

#[cfg(not(windows))]
pub fn enumerate_displays() -> Result<Vec<DisplayInfo>, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

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
pub fn enumerate_modes_for_display(_display_id: &str) -> Result<Vec<DisplayMode>, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn apply_mode(_request: &DisplayMode) -> Result<DisplayMode, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn test_current_display_mode(
    _display_id: Option<&str>,
) -> Result<DisplayModeTestResult, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn restore_mode(_pending: &PendingDisplayRestore) -> Result<DisplayMode, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn get_display_scale(_display_id: &str) -> Result<DisplayScale, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn set_display_scale(_display_id: &str, _scale: u32) -> Result<DisplayScale, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn get_hdr_state(_display_id: &str) -> Result<HdrState, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn set_hdr_enabled(_display_id: &str, _enabled: bool) -> Result<HdrState, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn capture_hdr_state(_display_id: &str) -> Result<HdrSnapshot, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn restore_hdr_state(_snapshot: &HdrSnapshot) -> Result<HdrState, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn capture_display_snapshot(_display_id: &str) -> Result<DisplaySnapshot, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}

#[cfg(not(windows))]
pub fn restore_display_snapshot(_snapshot: &DisplaySnapshot) -> Result<DisplaySnapshot, String> {
    Err("DISPLAY_WINDOWS_ONLY".to_string())
}
