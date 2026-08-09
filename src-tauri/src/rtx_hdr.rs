use crate::hardware_capabilities::{HardwareCapabilities, HardwareVendor};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const RTX_HDR_PEAK_NITS_DEFAULT: u32 = 800;
pub const RTX_HDR_MIDDLE_GREY_DEFAULT: u32 = 60;
pub const RTX_HDR_SATURATION_DEFAULT: u32 = 100;

pub(crate) fn contrast_raw_from_display(value: i32) -> Result<u32, String> {
    if !(-100..=100).contains(&value) {
        return Err("RTX_HDR_CONTRAST_OUT_OF_RANGE".to_string());
    }
    Ok((value + 100) as u32)
}

pub(crate) fn display_from_contrast_raw(value: u32) -> Result<i32, String> {
    if value > 200 {
        return Err("RTX_HDR_CONTRAST_RAW_OUT_OF_RANGE".to_string());
    }
    Ok(value as i32 - 100)
}

pub(crate) fn saturation_raw_from_display(value: i32) -> Result<u32, String> {
    if !(-100..=100).contains(&value) {
        return Err("RTX_HDR_SATURATION_OUT_OF_RANGE".to_string());
    }
    Ok((value + 100) as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RtxHdrPreset {
    Natural,
    Vibrant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxHdrSettingSnapshot {
    pub name: String,
    pub id: u32,
    pub existed: bool,
    pub setting_type: u32,
    pub location: u32,
    pub is_current_predefined: u32,
    pub is_predefined_valid: u32,
    pub predefined_value: Option<u32>,
    pub current_value: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxHdrProfileSnapshot {
    pub schema: u32,
    pub executable: String,
    pub profile_name: String,
    pub application_name: String,
    pub settings: Vec<RtxHdrSettingSnapshot>,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxHdrProfileState {
    pub supported: bool,
    pub executable: String,
    pub profile_name: Option<String>,
    pub application_name: Option<String>,
    pub settings: Vec<RtxHdrSettingSnapshot>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxHdrApplyResult {
    pub snapshot: RtxHdrProfileSnapshot,
    pub readback: RtxHdrProfileState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxHdrAvailability {
    pub supported: bool,
    pub nvidia_gpu: bool,
    pub nvidia_app_installed: bool,
    pub overlay_game_filters: Option<bool>,
    pub warnings: Vec<String>,
}

enum RestoreAction {
    Delete,
    Set(u32),
}

fn restore_action(saved: &RtxHdrSettingSnapshot) -> Result<RestoreAction, String> {
    if !saved.existed || saved.location != 0 || saved.setting_type != 0 {
        return Ok(RestoreAction::Delete);
    }
    Ok(RestoreAction::Set(saved.current_value.ok_or_else(
        || format!("RTX_HDR_SNAPSHOT_VALUE_MISSING:0x{:08X}", saved.id),
    )?))
}

#[tauri::command]
pub fn get_rtx_hdr_profile(executable: String) -> Result<RtxHdrProfileState, String> {
    get_profile(Path::new(&executable))
}

#[tauri::command]
pub fn apply_rtx_hdr_profile(
    executable: String,
    preset: RtxHdrPreset,
    peak_nits: Option<u32>,
) -> Result<RtxHdrApplyResult, String> {
    apply_profile(
        Path::new(&executable),
        preset,
        peak_nits.unwrap_or(RTX_HDR_PEAK_NITS_DEFAULT),
    )
}

#[tauri::command]
pub fn restore_rtx_hdr_profile(
    snapshot: RtxHdrProfileSnapshot,
) -> Result<RtxHdrProfileState, String> {
    restore_profile(&snapshot)
}

pub(crate) fn capture_for_launch(executable: &Path) -> Result<RtxHdrProfileSnapshot, String> {
    #[cfg(windows)]
    {
        return windows_impl::capture_profile(executable);
    }
    #[cfg(not(windows))]
    {
        let _ = executable;
        Err("RTX_HDR_WINDOWS_ONLY".to_string())
    }
}

pub(crate) fn apply_for_launch(
    executable: &Path,
    preset: RtxHdrPreset,
    peak_nits: u32,
) -> Result<RtxHdrApplyResult, String> {
    apply_profile(executable, preset, peak_nits)
}

pub(crate) fn restore_for_launch(
    snapshot: &RtxHdrProfileSnapshot,
) -> Result<RtxHdrProfileState, String> {
    restore_profile(snapshot)
}

#[tauri::command]
pub fn get_rtx_hdr_availability() -> RtxHdrAvailability {
    detect_availability(&crate::hardware_capabilities::cached_for_launch())
}

pub fn is_compatible_hardware(hardware: &HardwareCapabilities) -> bool {
    hardware.vendor == HardwareVendor::Nvidia
        && (hardware.feature_support.supports_dlss
            == crate::hardware_capabilities::FeatureSupport::Supported
            || hardware
                .model
                .as_deref()
                .is_some_and(|model| model.to_ascii_lowercase().contains("rtx")))
        && hardware
            .preferred_gaming_gpu
            .as_ref()
            .map(|gpu| gpu.vendor == HardwareVendor::Nvidia)
            .unwrap_or(true)
}

pub fn detect_availability(hardware: &HardwareCapabilities) -> RtxHdrAvailability {
    let nvidia_gpu = is_compatible_hardware(hardware);
    let nvidia_app_installed = nvidia_app_installed();
    let mut warnings = Vec::new();
    if !nvidia_gpu {
        warnings.push("RTX HDR requiere una GPU NVIDIA compatible.".to_string());
    }
    if !nvidia_app_installed {
        warnings.push("NVIDIA App no está instalado.".to_string());
    }
    warnings.push(
        "Overlay y Game Filters deben estar habilitados en NVIDIA App; LumaDeck no los automatiza."
            .to_string(),
    );
    RtxHdrAvailability {
        supported: nvidia_gpu && nvidia_app_installed,
        nvidia_gpu,
        nvidia_app_installed,
        overlay_game_filters: None,
        warnings,
    }
}

#[cfg(windows)]
fn get_profile(executable: &Path) -> Result<RtxHdrProfileState, String> {
    windows_impl::get_profile(executable)
}

#[cfg(windows)]
fn apply_profile(
    executable: &Path,
    preset: RtxHdrPreset,
    peak_nits: u32,
) -> Result<RtxHdrApplyResult, String> {
    windows_impl::apply_profile(executable, preset, peak_nits)
}

#[cfg(windows)]
fn restore_profile(snapshot: &RtxHdrProfileSnapshot) -> Result<RtxHdrProfileState, String> {
    windows_impl::restore_profile(snapshot)
}

fn nvidia_app_installed() -> bool {
    #[cfg(windows)]
    {
        let candidates = [
            std::env::var_os("ProgramFiles")
                .map(std::path::PathBuf::from)
                .map(|root| root.join("NVIDIA Corporation\\NVIDIA App\\CEF\\NVIDIA App.exe")),
            std::env::var_os("ProgramFiles(x86)")
                .map(std::path::PathBuf::from)
                .map(|root| root.join("NVIDIA Corporation\\NVIDIA App\\CEF\\NVIDIA App.exe")),
        ];
        candidates.into_iter().flatten().any(|path| path.is_file())
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::{
        ffi::{c_void, OsStr},
        fs,
        mem::transmute,
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
    };
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    const SETTING_STRUCT_SIZE: usize = 12_320;
    const APPLICATION_STRUCT_SIZE: usize = 20_492;
    const PROFILE_STRUCT_SIZE: usize = 4_116;
    const SETTING_TYPE_OFFSET: usize = 4_104;
    const SETTING_LOCATION_OFFSET: usize = 4_108;
    const SETTING_CURRENT_PREDEFINED_OFFSET: usize = 4_112;
    const SETTING_PREDEFINED_VALID_OFFSET: usize = 4_116;
    const SETTING_PREDEFINED_VALUE_OFFSET: usize = 4_120;
    const SETTING_CURRENT_VALUE_OFFSET: usize = 8_220;
    const APPLICATION_NAME_OFFSET: usize = 8;
    const NVAPI_OK: i32 = 0;
    const NVAPI_END_ENUMERATION: i32 = -7;
    const NVAPI_SETTING_NOT_FOUND: i32 = -160;
    const NVAPI_INITIALIZE: u32 = 0x0150_E828;
    const DRS_CREATE_SESSION: u32 = 0x0694_D52E;
    const DRS_DESTROY_SESSION: u32 = 0xDAD9_CFF8;
    const DRS_LOAD_SETTINGS: u32 = 0x375D_BD6B;
    const DRS_SAVE_SETTINGS: u32 = 0xFCBC_7E14;
    const DRS_FIND_APPLICATION_BY_NAME: u32 = 0xEEE5_66B2;
    const DRS_GET_PROFILE_INFO: u32 = 0x61CD_6FD6;
    const DRS_GET_SETTING_RAW: u32 = 0xEA99_498D;
    const DRS_SET_SETTING_RAW: u32 = 0x8A2C_F5F5;
    const DRS_DELETE_PROFILE_SETTING: u32 = 0xE4A2_6362;
    const RTX_HDR_ENABLE_ID: u32 = 0x00DD_48FB;
    const RTX_HDR_PEAK_BRIGHTNESS_ID: u32 = 0x00DD_48FC;
    const RTX_HDR_MIDDLE_GREY_ID: u32 = 0x00DD_48FD;
    const RTX_HDR_CONTRAST_ID: u32 = 0x00DD_48FE;
    const RTX_HDR_SATURATION_ID: u32 = 0x00DD_48FF;
    const RTX_HDR_DEBANDING_ID: u32 = 0x0043_2F84;

    #[derive(Clone, Copy)]
    struct Api {
        _module: HMODULE,
        initialize: unsafe extern "C" fn() -> i32,
        create_session: unsafe extern "C" fn(*mut *mut c_void) -> i32,
        destroy_session: unsafe extern "C" fn(*mut c_void) -> i32,
        load_settings: unsafe extern "C" fn(*mut c_void) -> i32,
        save_settings: unsafe extern "C" fn(*mut c_void) -> i32,
        find_application:
            unsafe extern "C" fn(*mut c_void, *const u16, *mut *mut c_void, *mut u8) -> i32,
        get_profile_info: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u8) -> i32,
        get_setting: unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *mut u8, *mut u32) -> i32,
        set_setting: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u8, u32, u32) -> i32,
        delete_profile_setting: unsafe extern "C" fn(*mut c_void, *mut c_void, u32) -> i32,
    }

    struct Session<'a> {
        api: &'a Api,
        handle: *mut c_void,
    }

    impl Drop for Session<'_> {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { (self.api.destroy_session)(self.handle) };
            }
        }
    }

    #[derive(Clone, Copy)]
    struct TargetSetting {
        name: &'static str,
        id: u32,
    }

    const TARGET_SETTINGS: [TargetSetting; 6] = [
        TargetSetting {
            name: "RTX HDR Enable",
            id: RTX_HDR_ENABLE_ID,
        },
        TargetSetting {
            name: "Peak Brightness",
            id: RTX_HDR_PEAK_BRIGHTNESS_ID,
        },
        TargetSetting {
            name: "Middle Grey",
            id: RTX_HDR_MIDDLE_GREY_ID,
        },
        TargetSetting {
            name: "Contrast",
            id: RTX_HDR_CONTRAST_ID,
        },
        TargetSetting {
            name: "Saturation",
            id: RTX_HDR_SATURATION_ID,
        },
        TargetSetting {
            name: "Debanding",
            id: RTX_HDR_DEBANDING_ID,
        },
    ];

    struct Target {
        profile: *mut c_void,
        profile_name: String,
        application_name: String,
    }

    #[derive(Clone, Copy)]
    struct SettingRead {
        existed: bool,
        setting_type: u32,
        location: u32,
        is_current_predefined: u32,
        is_predefined_valid: u32,
        predefined_value: Option<u32>,
        current_value: Option<u32>,
    }

    pub(super) fn get_profile(executable: &Path) -> Result<RtxHdrProfileState, String> {
        let api = Api::load()?;
        let session = Session::open(&api)?;
        let target = Target::lookup(&session, executable)?;
        let settings = read_settings(&session, &target)?;
        Ok(RtxHdrProfileState {
            supported: true,
            executable: executable.display().to_string(),
            profile_name: Some(target.profile_name),
            application_name: Some(target.application_name),
            settings,
            diagnostic: None,
        })
    }

    pub(super) fn apply_profile(
        executable: &Path,
        preset: RtxHdrPreset,
        peak_nits: u32,
    ) -> Result<RtxHdrApplyResult, String> {
        if peak_nits == 0 || peak_nits > 10_000 {
            return Err("RTX_HDR_PEAK_NITS_INVALID".to_string());
        }
        let api = Api::load()?;
        let session = Session::open(&api)?;
        let target = Target::lookup(&session, executable)?;
        let snapshot = capture_snapshot(&session, &target, executable)?;
        let apply_result = apply_settings(&session, &target, preset, peak_nits).and_then(|_| {
            check_status(
                unsafe { (session.api.save_settings)(session.handle) },
                "NvAPI_DRS_SaveSettings",
            )
        });
        if let Err(error) = apply_result {
            let _ = restore_settings(&session, &target, &snapshot);
            return Err(error);
        }
        drop(session);
        let reopened_api = Api::load()?;
        let reopened = Session::open(&reopened_api)?;
        let reopened_target = Target::lookup(&reopened, executable)?;
        let readback = state_from_target(&reopened, &reopened_target, executable)?;
        verify_applied(&readback, preset, peak_nits)?;
        Ok(RtxHdrApplyResult { snapshot, readback })
    }

    pub(super) fn capture_profile(executable: &Path) -> Result<RtxHdrProfileSnapshot, String> {
        let api = Api::load()?;
        let session = Session::open(&api)?;
        let target = Target::lookup(&session, executable)?;
        capture_snapshot(&session, &target, executable)
    }

    pub(super) fn restore_profile(
        snapshot: &RtxHdrProfileSnapshot,
    ) -> Result<RtxHdrProfileState, String> {
        let executable = PathBuf::from(&snapshot.executable);
        let api = Api::load()?;
        let session = Session::open(&api)?;
        let target = Target::lookup(&session, &executable)?;
        restore_settings(&session, &target, snapshot)?;
        drop(session);
        let reopened_api = Api::load()?;
        let reopened = Session::open(&reopened_api)?;
        let reopened_target = Target::lookup(&reopened, &executable)?;
        state_from_target(&reopened, &reopened_target, &executable)
    }

    impl Api {
        fn load() -> Result<Self, String> {
            let wide: Vec<u16> = OsStr::new("nvapi64.dll")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let module = unsafe { LoadLibraryW(wide.as_ptr()) };
            if module.is_null() {
                return Err("RTX_HDR_NVAPI_DLL_UNAVAILABLE".to_string());
            }
            let query = unsafe { GetProcAddress(module, b"nvapi_QueryInterface\0".as_ptr()) }
                .ok_or_else(|| "RTX_HDR_NVAPI_QUERY_UNAVAILABLE".to_string())?;
            let query: unsafe extern "C" fn(u32) -> *const c_void = unsafe { transmute(query) };
            let get = |id: u32, name: &str| -> Result<*const c_void, String> {
                let pointer = unsafe { query(id) };
                if pointer.is_null() {
                    Err(format!("RTX_HDR_NVAPI_FUNCTION_UNAVAILABLE:{name}"))
                } else {
                    Ok(pointer)
                }
            };
            let api = Self {
                _module: module,
                initialize: unsafe { transmute(get(NVAPI_INITIALIZE, "NvAPI_Initialize")?) },
                create_session: unsafe {
                    transmute(get(DRS_CREATE_SESSION, "NvAPI_DRS_CreateSession")?)
                },
                destroy_session: unsafe {
                    transmute(get(DRS_DESTROY_SESSION, "NvAPI_DRS_DestroySession")?)
                },
                load_settings: unsafe {
                    transmute(get(DRS_LOAD_SETTINGS, "NvAPI_DRS_LoadSettings")?)
                },
                save_settings: unsafe {
                    transmute(get(DRS_SAVE_SETTINGS, "NvAPI_DRS_SaveSettings")?)
                },
                find_application: unsafe {
                    transmute(get(
                        DRS_FIND_APPLICATION_BY_NAME,
                        "NvAPI_DRS_FindApplicationByName",
                    )?)
                },
                get_profile_info: unsafe {
                    transmute(get(DRS_GET_PROFILE_INFO, "NvAPI_DRS_GetProfileInfo")?)
                },
                get_setting: unsafe {
                    transmute(get(DRS_GET_SETTING_RAW, "NvAPI_DRS_GetSetting(raw)")?)
                },
                set_setting: unsafe {
                    transmute(get(DRS_SET_SETTING_RAW, "NvAPI_DRS_SetSetting(raw)")?)
                },
                delete_profile_setting: unsafe {
                    transmute(get(
                        DRS_DELETE_PROFILE_SETTING,
                        "NvAPI_DRS_DeleteProfileSetting",
                    )?)
                },
            };
            check_status(unsafe { (api.initialize)() }, "NvAPI_Initialize")?;
            Ok(api)
        }
    }

    impl<'a> Session<'a> {
        fn open(api: &'a Api) -> Result<Self, String> {
            let mut handle = std::ptr::null_mut();
            check_status(
                unsafe { (api.create_session)(&mut handle) },
                "NvAPI_DRS_CreateSession",
            )?;
            let session = Self { api, handle };
            if let Err(error) = check_status(
                unsafe { (api.load_settings)(handle) },
                "NvAPI_DRS_LoadSettings",
            ) {
                drop(session);
                return Err(error);
            }
            Ok(session)
        }
    }

    impl Target {
        fn lookup(session: &Session<'_>, executable: &Path) -> Result<Self, String> {
            let canonical =
                fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
            let normalized = canonical.to_string_lossy().replace('\\', "/");
            let mut application_name = fixed_utf16(&normalized)?;
            let mut application = vec![0_u8; APPLICATION_STRUCT_SIZE];
            write_u32(&mut application, 0, version(APPLICATION_STRUCT_SIZE, 4));
            let mut profile = std::ptr::null_mut();
            let status = unsafe {
                (session.api.find_application)(
                    session.handle,
                    application_name.as_mut_ptr(),
                    &mut profile,
                    application.as_mut_ptr(),
                )
            };
            check_status(status, "NvAPI_DRS_FindApplicationByName")?;
            let mut profile_info = vec![0_u8; PROFILE_STRUCT_SIZE];
            write_u32(&mut profile_info, 0, version(PROFILE_STRUCT_SIZE, 1));
            check_status(
                unsafe {
                    (session.api.get_profile_info)(
                        session.handle,
                        profile,
                        profile_info.as_mut_ptr(),
                    )
                },
                "NvAPI_DRS_GetProfileInfo",
            )?;
            Ok(Self {
                profile,
                profile_name: read_utf16(&profile_info, 4, 2_048),
                application_name: read_utf16(&application, APPLICATION_NAME_OFFSET, 2_048),
            })
        }
    }

    fn capture_snapshot(
        session: &Session<'_>,
        target: &Target,
        executable: &Path,
    ) -> Result<RtxHdrProfileSnapshot, String> {
        Ok(RtxHdrProfileSnapshot {
            schema: 1,
            executable: executable.display().to_string(),
            profile_name: target.profile_name.clone(),
            application_name: target.application_name.clone(),
            settings: read_settings(session, target)?,
            captured_at: timestamp(),
        })
    }

    fn read_settings(
        session: &Session<'_>,
        target: &Target,
    ) -> Result<Vec<RtxHdrSettingSnapshot>, String> {
        TARGET_SETTINGS
            .iter()
            .map(|setting| {
                let read = read_setting(session, target.profile, setting.id)?;
                Ok(RtxHdrSettingSnapshot {
                    name: setting.name.to_string(),
                    id: setting.id,
                    existed: read.existed,
                    setting_type: read.setting_type,
                    location: read.location,
                    is_current_predefined: read.is_current_predefined,
                    is_predefined_valid: read.is_predefined_valid,
                    predefined_value: read.predefined_value,
                    current_value: read.current_value,
                })
            })
            .collect()
    }

    fn state_from_target(
        session: &Session<'_>,
        target: &Target,
        executable: &Path,
    ) -> Result<RtxHdrProfileState, String> {
        Ok(RtxHdrProfileState {
            supported: true,
            executable: executable.display().to_string(),
            profile_name: Some(target.profile_name.clone()),
            application_name: Some(target.application_name.clone()),
            settings: read_settings(session, target)?,
            diagnostic: None,
        })
    }

    fn apply_settings(
        session: &Session<'_>,
        target: &Target,
        preset: RtxHdrPreset,
        peak_nits: u32,
    ) -> Result<(), String> {
        let _ = preset;
        set_dword(session, target.profile, RTX_HDR_ENABLE_ID, 1)?;
        set_dword(
            session,
            target.profile,
            RTX_HDR_PEAK_BRIGHTNESS_ID,
            peak_nits,
        )?;
        set_dword(
            session,
            target.profile,
            RTX_HDR_MIDDLE_GREY_ID,
            RTX_HDR_MIDDLE_GREY_DEFAULT,
        )?;
        set_dword(
            session,
            target.profile,
            RTX_HDR_CONTRAST_ID,
            contrast_raw_from_display(15)?,
        )?;
        set_dword(
            session,
            target.profile,
            RTX_HDR_SATURATION_ID,
            saturation_raw_from_display(0)?,
        )?;
        Ok(())
    }

    fn verify_applied(
        state: &RtxHdrProfileState,
        _preset: RtxHdrPreset,
        peak_nits: u32,
    ) -> Result<(), String> {
        let value = |id: u32| {
            state
                .settings
                .iter()
                .find(|setting| setting.id == id)
                .and_then(|setting| setting.current_value)
        };
        if value(RTX_HDR_ENABLE_ID) != Some(1)
            || value(RTX_HDR_PEAK_BRIGHTNESS_ID) != Some(peak_nits)
            || value(RTX_HDR_MIDDLE_GREY_ID) != Some(RTX_HDR_MIDDLE_GREY_DEFAULT)
            || value(RTX_HDR_CONTRAST_ID).and_then(|raw| display_from_contrast_raw(raw).ok())
                != Some(15)
            || value(RTX_HDR_SATURATION_ID) != Some(RTX_HDR_SATURATION_DEFAULT)
        {
            return Err("RTX_HDR_VERIFY_FAILED".to_string());
        }
        Ok(())
    }

    fn restore_settings(
        session: &Session<'_>,
        target: &Target,
        snapshot: &RtxHdrProfileSnapshot,
    ) -> Result<(), String> {
        for saved in &snapshot.settings {
            match restore_action(saved)? {
                RestoreAction::Delete => delete_setting(session, target.profile, saved.id)?,
                RestoreAction::Set(value) => set_dword(session, target.profile, saved.id, value)?,
            }
        }
        check_status(
            unsafe { (session.api.save_settings)(session.handle) },
            "NvAPI_DRS_SaveSettings",
        )
    }

    fn read_setting(
        session: &Session<'_>,
        profile: *mut c_void,
        id: u32,
    ) -> Result<SettingRead, String> {
        let mut buffer = vec![0_u8; SETTING_STRUCT_SIZE];
        write_u32(&mut buffer, 0, version(SETTING_STRUCT_SIZE, 1));
        let mut extra_param = 0_u32;
        let status = unsafe {
            (session.api.get_setting)(
                session.handle,
                profile,
                id,
                buffer.as_mut_ptr(),
                &mut extra_param,
            )
        };
        if status == NVAPI_SETTING_NOT_FOUND {
            return Ok(SettingRead {
                existed: false,
                setting_type: 0,
                location: 0,
                is_current_predefined: 0,
                is_predefined_valid: 0,
                predefined_value: None,
                current_value: None,
            });
        }
        check_status(status, &format!("NvAPI_DRS_GetSetting(0x{id:08X})"))?;
        Ok(SettingRead {
            existed: true,
            setting_type: read_u32(&buffer, SETTING_TYPE_OFFSET),
            location: read_u32(&buffer, SETTING_LOCATION_OFFSET),
            is_current_predefined: read_u32(&buffer, SETTING_CURRENT_PREDEFINED_OFFSET),
            is_predefined_valid: read_u32(&buffer, SETTING_PREDEFINED_VALID_OFFSET),
            predefined_value: Some(read_u32(&buffer, SETTING_PREDEFINED_VALUE_OFFSET)),
            current_value: Some(read_u32(&buffer, SETTING_CURRENT_VALUE_OFFSET)),
        })
    }

    fn set_dword(
        session: &Session<'_>,
        profile: *mut c_void,
        id: u32,
        value: u32,
    ) -> Result<(), String> {
        let mut buffer = vec![0_u8; SETTING_STRUCT_SIZE];
        write_u32(&mut buffer, 0, version(SETTING_STRUCT_SIZE, 1));
        write_u32(&mut buffer, 4_100, id);
        write_u32(&mut buffer, SETTING_TYPE_OFFSET, 0);
        write_u32(&mut buffer, SETTING_LOCATION_OFFSET, 0);
        write_u32(&mut buffer, SETTING_CURRENT_VALUE_OFFSET, value);
        check_status(
            unsafe {
                (session.api.set_setting)(session.handle, profile, buffer.as_mut_ptr(), 0, 0)
            },
            &format!("NvAPI_DRS_SetSetting(raw, 0x{id:08X})"),
        )
    }

    fn delete_setting(session: &Session<'_>, profile: *mut c_void, id: u32) -> Result<(), String> {
        let status = unsafe { (session.api.delete_profile_setting)(session.handle, profile, id) };
        if status != NVAPI_OK && status != NVAPI_SETTING_NOT_FOUND && status != -137 {
            return Err(format_status(
                status,
                &format!("NvAPI_DRS_DeleteProfileSetting(0x{id:08X})"),
            ));
        }
        Ok(())
    }

    fn fixed_utf16(value: &str) -> Result<Vec<u16>, String> {
        let mut result = vec![0_u16; 2_048];
        let encoded: Vec<u16> = value.encode_utf16().collect();
        if encoded.len() >= result.len() {
            return Err("RTX_HDR_EXECUTABLE_PATH_TOO_LONG".to_string());
        }
        result[..encoded.len()].copy_from_slice(&encoded);
        Ok(result)
    }

    fn read_utf16(bytes: &[u8], offset: usize, capacity: usize) -> String {
        let mut values = Vec::new();
        for index in 0..capacity {
            let position = offset + index * 2;
            if position + 1 >= bytes.len() {
                break;
            }
            let value = u16::from_ne_bytes([bytes[position], bytes[position + 1]]);
            if value == 0 {
                break;
            }
            values.push(value);
        }
        String::from_utf16_lossy(&values)
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_ne_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("fixed NVAPI offset"),
        )
    }
    fn version(size: usize, revision: u32) -> u32 {
        size as u32 | (revision << 16)
    }
    fn check_status(status: i32, operation: &str) -> Result<(), String> {
        if status == NVAPI_OK {
            Ok(())
        } else {
            Err(format_status(status, operation))
        }
    }
    fn format_status(status: i32, operation: &str) -> String {
        let name = if status == NVAPI_END_ENUMERATION {
            "NVAPI_END_ENUMERATION"
        } else if status == NVAPI_SETTING_NOT_FOUND {
            "NVAPI_SETTING_NOT_FOUND"
        } else {
            "NVAPI_STATUS_UNKNOWN"
        };
        format!("{operation} returned {name} ({status})")
    }
}

#[cfg(not(windows))]
fn get_profile(executable: &Path) -> Result<RtxHdrProfileState, String> {
    Ok(RtxHdrProfileState {
        supported: false,
        executable: executable.display().to_string(),
        profile_name: None,
        application_name: None,
        settings: Vec::new(),
        diagnostic: Some("RTX_HDR_WINDOWS_ONLY".to_string()),
    })
}

#[cfg(not(windows))]
fn apply_profile(
    executable: &Path,
    _preset: RtxHdrPreset,
    _peak_nits: u32,
) -> Result<RtxHdrApplyResult, String> {
    Err(format!("RTX_HDR_WINDOWS_ONLY:{}", executable.display()))
}

#[cfg(not(windows))]
fn restore_profile(snapshot: &RtxHdrProfileSnapshot) -> Result<RtxHdrProfileState, String> {
    Err(format!("RTX_HDR_WINDOWS_ONLY:{}", snapshot.executable))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_values_are_stable() {
        assert_eq!(
            serde_json::to_string(&RtxHdrPreset::Natural).unwrap(),
            "\"NATURAL\""
        );
        assert_eq!(
            serde_json::to_string(&RtxHdrPreset::Vibrant).unwrap(),
            "\"VIBRANT\""
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_inherited_setting() {
        let snapshot = RtxHdrProfileSnapshot {
            schema: 1,
            executable: "game.exe".to_string(),
            profile_name: "Game".to_string(),
            application_name: "game.exe".to_string(),
            captured_at: "1".to_string(),
            settings: vec![RtxHdrSettingSnapshot {
                name: "Debanding".to_string(),
                id: 0x0043_2F84,
                existed: false,
                setting_type: 0,
                location: 0,
                is_current_predefined: 0,
                is_predefined_valid: 0,
                predefined_value: None,
                current_value: None,
            }],
        };
        let decoded: RtxHdrProfileSnapshot =
            serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
        assert_eq!(decoded, snapshot);
        assert!(!decoded.settings[0].existed);
    }

    #[test]
    fn contrast_and_saturation_use_raw_offset_encoding() {
        assert_eq!(contrast_raw_from_display(15).unwrap(), 115);
        assert_eq!(display_from_contrast_raw(115).unwrap(), 15);
        assert_eq!(saturation_raw_from_display(0).unwrap(), 100);
    }

    #[test]
    fn restore_policy_deletes_inherited_or_missing_settings() {
        let missing = RtxHdrSettingSnapshot {
            name: "Debanding".to_string(),
            id: 1,
            existed: false,
            setting_type: 0,
            location: 0,
            is_current_predefined: 0,
            is_predefined_valid: 0,
            predefined_value: None,
            current_value: None,
        };
        let inherited = RtxHdrSettingSnapshot {
            existed: true,
            location: 2,
            current_value: Some(1),
            ..missing.clone()
        };
        assert!(matches!(
            super::restore_action(&missing).unwrap(),
            super::RestoreAction::Delete
        ));
        assert!(matches!(
            super::restore_action(&inherited).unwrap(),
            super::RestoreAction::Delete
        ));
        let explicit = RtxHdrSettingSnapshot {
            existed: true,
            location: 0,
            setting_type: 0,
            current_value: Some(115),
            ..missing
        };
        assert!(matches!(
            super::restore_action(&explicit).unwrap(),
            super::RestoreAction::Set(115)
        ));
    }
}
