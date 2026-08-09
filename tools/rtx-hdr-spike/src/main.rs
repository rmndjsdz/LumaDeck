#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::fs;
use std::mem::transmute;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

const DEFAULT_JEDI_EXE: &str = r"F:\SteamLibrary\steamapps\common\Jedi Fallen Order\SwGame\Binaries\Win64\starwarsjedifallenorder.exe";
const SETTING_STRUCT_SIZE: usize = 12_320;
const APPLICATION_STRUCT_SIZE: usize = 20_492;
const PROFILE_STRUCT_SIZE: usize = 4_116;

const SETTING_ID_OFFSET: usize = 4_100;
const SETTING_TYPE_OFFSET: usize = 4_104;
const SETTING_LOCATION_OFFSET: usize = 4_108;
const SETTING_CURRENT_PREDEFINED_OFFSET: usize = 4_112;
const SETTING_PREDEFINED_VALID_OFFSET: usize = 4_116;
const SETTING_PREDEFINED_VALUE_OFFSET: usize = 4_120;
const SETTING_CURRENT_VALUE_OFFSET: usize = 8_220;

const APPLICATION_VERSION_OFFSET: usize = 0;
const APPLICATION_NAME_OFFSET: usize = 8;

const NVAPI_OK: i32 = 0;
const NVAPI_END_ENUMERATION: i32 = -7;
const NVAPI_SETTING_NOT_FOUND: i32 = -160;
const NVAPI_PROFILE_NOT_FOUND: i32 = -163;
const NVAPI_APPLICATION_NOT_FOUND: i32 = -180;
const NVAPI_EXECUTABLE_NOT_FOUND: i32 = -166;
const NVAPI_ACCESS_DENIED: i32 = -175;

const NVAPI_INITIALIZE: u32 = 0x0150_E828;
const DRS_CREATE_SESSION: u32 = 0x0694_D52E;
const DRS_DESTROY_SESSION: u32 = 0xDAD9_CFF8;
const DRS_LOAD_SETTINGS: u32 = 0x375D_BD6B;
const DRS_SAVE_SETTINGS: u32 = 0xFCBC_7E14;
const DRS_FIND_APPLICATION_BY_NAME: u32 = 0xEEE5_66B2;
const DRS_ENUM_PROFILES: u32 = 0xBC37_1EE0;
const DRS_ENUM_APPLICATIONS: u32 = 0x7FA2_173A;
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
    enum_profiles: unsafe extern "C" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    enum_applications:
        unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *mut u32, *mut u8) -> i32,
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

#[derive(Debug, Clone)]
struct SettingRead {
    status: i32,
    existed: bool,
    setting_type: u32,
    location: u32,
    is_current_predefined: u32,
    is_predefined_valid: u32,
    predefined_value: Option<u32>,
    current_value: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    schema: u32,
    exe_path: String,
    profile_name: String,
    application_name: String,
    settings: Vec<SnapshotSetting>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotSetting {
    name: String,
    id: u32,
    existed: bool,
    setting_type: u32,
    location: u32,
    is_current_predefined: u32,
    is_predefined_valid: u32,
    predefined_value: Option<u32>,
    current_value: Option<u32>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("RTX_HDR_DRS_ERROR: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("get");
    if matches!(command, "help" | "--help" | "-h") {
        print_usage();
        return Ok(());
    }

    let exe_path = option_value(&args, "--exe")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_JEDI_EXE));
    let snapshot_path = option_value(&args, "--snapshot")
        .map(PathBuf::from)
        .unwrap_or_else(default_snapshot_path);

    if !exe_path.is_file() {
        return Err(format!("Jedi executable not found: {}", exe_path.display()));
    }

    let api = Api::load()?;
    let session = Session::open(&api)?;
    if command == "mappings" {
        return print_mappings(&session, &exe_path);
    }
    let target = Target::lookup(&session, &exe_path)?;

    return match command {
        "get" => print_get(&session, &target),
        "set-contrast" => {
            let display_value = parse_i32_arg(&args, 2, "contrast display value")?;
            if !(-100..=100).contains(&display_value) {
                return Err(
                    "contrast display value must be in the range -100..100 (stored value 0..200)"
                        .to_string(),
                );
            }
            let stored_value = (100 + display_value) as u32;
            let snapshot = capture_snapshot(&session, &target, &exe_path)?;
            write_snapshot(&snapshot_path, &snapshot)?;
            println!("SNAPSHOT_WRITTEN={}", snapshot_path.display());
            let before = read_required(&session, target.profile, RTX_HDR_CONTRAST_ID, "Contrast")?;
            set_dword(&session, target.profile, RTX_HDR_CONTRAST_ID, stored_value)?;
            println!(
                "SET_CONTRAST before_raw={} after_raw={} after_display={}",
                value_or_dash(before.current_value),
                stored_value,
                stored_value as i32 - 100
            );
            drop(session);
            let reopened = Session::open(&api)?;
            let reopened_target = Target::lookup(&reopened, &exe_path)?;
            let after = read_required(
                &reopened,
                reopened_target.profile,
                RTX_HDR_CONTRAST_ID,
                "Contrast",
            )?;
            println!(
                "NEW_SESSION_GET contrast_raw={} contrast_display={}",
                value_or_dash(after.current_value),
                display_value_for(RTX_HDR_CONTRAST_ID, after.current_value)
            );
            Ok(())
        }
        "restore-contrast" => {
            let snapshot = read_snapshot(&snapshot_path)?;
            restore_one(&session, &target, &snapshot, RTX_HDR_CONTRAST_ID)?;
            println!(
                "RESTORE_CONTRAST completed from {}",
                snapshot_path.display()
            );
            drop(session);
            let reopened = Session::open(&api)?;
            let reopened_target = Target::lookup(&reopened, &exe_path)?;
            let after = read_required(
                &reopened,
                reopened_target.profile,
                RTX_HDR_CONTRAST_ID,
                "Contrast",
            )?;
            println!(
                "NEW_SESSION_GET contrast_raw={} contrast_display={}",
                value_or_dash(after.current_value),
                display_value_for(RTX_HDR_CONTRAST_ID, after.current_value)
            );
            Ok(())
        }
        "toggle-off" => {
            let snapshot = capture_snapshot(&session, &target, &exe_path)?;
            write_snapshot(&snapshot_path, &snapshot)?;
            println!("SNAPSHOT_WRITTEN={}", snapshot_path.display());
            let before = read_required(
                &session,
                target.profile,
                RTX_HDR_ENABLE_ID,
                "RTX HDR Enable",
            )?;
            set_dword(&session, target.profile, RTX_HDR_ENABLE_ID, 0)?;
            println!(
                "TOGGLE before_raw={} after_raw=0",
                value_or_dash(before.current_value)
            );
            drop(session);
            let reopened = Session::open(&api)?;
            let reopened_target = Target::lookup(&reopened, &exe_path)?;
            let after = read_required(
                &reopened,
                reopened_target.profile,
                RTX_HDR_ENABLE_ID,
                "RTX HDR Enable",
            )?;
            println!(
                "NEW_SESSION_GET rtx_hdr_enable_raw={} enabled={}",
                value_or_dash(after.current_value),
                is_on(after.current_value)
            );
            Ok(())
        }
        "toggle-restore" => {
            let snapshot = read_snapshot(&snapshot_path)?;
            for setting in &snapshot.settings {
                restore_one(&session, &target, &snapshot, setting.id)?;
            }
            println!("TOGGLE_RESTORE completed from {}", snapshot_path.display());
            drop(session);
            let reopened = Session::open(&api)?;
            let reopened_target = Target::lookup(&reopened, &exe_path)?;
            let after = read_required(
                &reopened,
                reopened_target.profile,
                RTX_HDR_ENABLE_ID,
                "RTX HDR Enable",
            )?;
            println!(
                "NEW_SESSION_GET rtx_hdr_enable_raw={} enabled={}",
                value_or_dash(after.current_value),
                is_on(after.current_value)
            );
            Ok(())
        }
        _ => return Err(format!("unknown command '{command}' (use --help)")),
    };
}

impl Api {
    fn load() -> Result<Self, String> {
        let wide: Vec<u16> = "nvapi64.dll"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            return Err("LoadLibraryW(nvapi64.dll) failed".to_string());
        }
        let query = unsafe { GetProcAddress(module, b"nvapi_QueryInterface\0".as_ptr()) }
            .ok_or_else(|| "GetProcAddress(nvapi_QueryInterface) failed".to_string())?;
        let query: unsafe extern "C" fn(u32) -> *const c_void = unsafe { transmute(query) };
        let get = |id: u32, name: &str| -> Result<*const c_void, String> {
            let ptr = unsafe { query(id) };
            if ptr.is_null() {
                Err(format!("NvAPI function {name} (0x{id:08X}) unavailable"))
            } else {
                Ok(ptr)
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
            load_settings: unsafe { transmute(get(DRS_LOAD_SETTINGS, "NvAPI_DRS_LoadSettings")?) },
            save_settings: unsafe { transmute(get(DRS_SAVE_SETTINGS, "NvAPI_DRS_SaveSettings")?) },
            find_application: unsafe {
                transmute(get(
                    DRS_FIND_APPLICATION_BY_NAME,
                    "NvAPI_DRS_FindApplicationByName",
                )?)
            },
            enum_profiles: unsafe { transmute(get(DRS_ENUM_PROFILES, "NvAPI_DRS_EnumProfiles")?) },
            enum_applications: unsafe {
                transmute(get(DRS_ENUM_APPLICATIONS, "NvAPI_DRS_EnumApplications")?)
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
        let status = unsafe { (api.initialize)() };
        check_status(status, "NvAPI_Initialize")?;
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

struct Target {
    profile: *mut c_void,
    profile_name: String,
    application_name: String,
    application_file_in_folder: String,
}

impl Target {
    fn lookup(session: &Session<'_>, exe_path: &Path) -> Result<Self, String> {
        let canonical =
            fs::canonicalize(exe_path).map_err(|error| format!("canonicalize target: {error}"))?;
        let normalized = canonical.to_string_lossy().replace('\\', "/");
        let mut app_name = fixed_utf16(&normalized)?;
        let mut application = vec![0_u8; APPLICATION_STRUCT_SIZE];
        write_u32(
            &mut application,
            APPLICATION_VERSION_OFFSET,
            version(APPLICATION_STRUCT_SIZE, 4),
        );
        let mut profile = std::ptr::null_mut();
        let status = unsafe {
            (session.api.find_application)(
                session.handle,
                app_name.as_mut_ptr(),
                &mut profile,
                application.as_mut_ptr(),
            )
        };
        if status != NVAPI_OK {
            return Err(format_status(status, "NvAPI_DRS_FindApplicationByName"));
        }

        let mut profile_info = vec![0_u8; PROFILE_STRUCT_SIZE];
        write_u32(&mut profile_info, 0, version(PROFILE_STRUCT_SIZE, 1));
        check_status(
            unsafe {
                (session.api.get_profile_info)(session.handle, profile, profile_info.as_mut_ptr())
            },
            "NvAPI_DRS_GetProfileInfo",
        )?;

        Ok(Self {
            profile,
            profile_name: read_utf16(&profile_info, 4, 2_048),
            application_name: read_utf16(&application, APPLICATION_NAME_OFFSET, 2_048),
            application_file_in_folder: read_utf16(&application, 12_296, 2_048),
        })
    }
}

fn capture_snapshot(
    session: &Session<'_>,
    target: &Target,
    exe_path: &Path,
) -> Result<Snapshot, String> {
    let mut settings = Vec::with_capacity(TARGET_SETTINGS.len());
    for target_setting in TARGET_SETTINGS {
        let read = read_setting(session, target.profile, target_setting.id)?;
        settings.push(SnapshotSetting {
            name: target_setting.name.to_string(),
            id: target_setting.id,
            existed: read.existed,
            setting_type: read.setting_type,
            location: read.location,
            is_current_predefined: read.is_current_predefined,
            is_predefined_valid: read.is_predefined_valid,
            predefined_value: read.predefined_value,
            current_value: read.current_value,
        });
    }
    Ok(Snapshot {
        schema: 1,
        exe_path: exe_path.display().to_string(),
        profile_name: target.profile_name.clone(),
        application_name: target.application_name.clone(),
        settings,
    })
}

fn print_get(session: &Session<'_>, target: &Target) -> Result<(), String> {
    println!("RTX_HDR_DRS_CONFIRMED_CANDIDATE");
    println!(
        "PROFILE name={:?} application={:?} file_in_folder={:?} scope=per-game",
        target.profile_name, target.application_name, target.application_file_in_folder
    );
    println!("SETTING | ID | TYPE | LOCATION | EXISTS | RAW | DISPLAY | PREDEFINED");
    for target_setting in TARGET_SETTINGS {
        let read = read_setting(session, target.profile, target_setting.id)?;
        println!(
            "{} | 0x{:08X} | {} | {} | {} | {} | {} | {}",
            target_setting.name,
            target_setting.id,
            type_name(read.setting_type),
            location_name(read.location),
            read.existed,
            value_or_dash(read.current_value),
            display_value_for(target_setting.id, read.current_value),
            value_or_dash(read.predefined_value),
        );
    }
    Ok(())
}

fn print_mappings(session: &Session<'_>, exe_path: &Path) -> Result<(), String> {
    let requested_name = exe_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    println!("APPLICATION_MAPPINGS requested_path={}", exe_path.display());
    for profile_index in 0..10_000_u32 {
        let mut profile = std::ptr::null_mut();
        let status =
            unsafe { (session.api.enum_profiles)(session.handle, profile_index, &mut profile) };
        if status == NVAPI_END_ENUMERATION {
            break;
        }
        check_status(status, &format!("NvAPI_DRS_EnumProfiles({profile_index})"))?;

        let mut profile_info = vec![0_u8; PROFILE_STRUCT_SIZE];
        write_u32(&mut profile_info, 0, version(PROFILE_STRUCT_SIZE, 1));
        check_status(
            unsafe {
                (session.api.get_profile_info)(session.handle, profile, profile_info.as_mut_ptr())
            },
            "NvAPI_DRS_GetProfileInfo",
        )?;
        let profile_name = read_utf16(&profile_info, 4, 2_048);
        let profile_is_jedi = profile_name.to_ascii_lowercase().contains("jedi");

        for app_index in 0..10_000_u32 {
            let mut app_count = 1_u32;
            let mut application = vec![0_u8; APPLICATION_STRUCT_SIZE];
            write_u32(
                &mut application,
                APPLICATION_VERSION_OFFSET,
                version(APPLICATION_STRUCT_SIZE, 4),
            );
            let app_status = unsafe {
                (session.api.enum_applications)(
                    session.handle,
                    profile,
                    app_index,
                    &mut app_count,
                    application.as_mut_ptr(),
                )
            };
            if app_status == NVAPI_END_ENUMERATION || app_count == 0 {
                break;
            }
            check_status(
                app_status,
                &format!("NvAPI_DRS_EnumApplications(profile={profile_name:?}, index={app_index})"),
            )?;
            let application_name = read_utf16(&application, APPLICATION_NAME_OFFSET, 2_048);
            if profile_is_jedi || application_name.to_ascii_lowercase() == requested_name {
                println!("MATCH profile={profile_name:?} app={application_name:?} file_in_folder={:?} launcher={:?}", read_utf16(&application, 12_296, 2_048), read_utf16(&application, 8_200, 2_048));
            }
        }
    }
    Ok(())
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
            status,
            existed: false,
            setting_type: 0,
            location: 0,
            is_current_predefined: 0,
            is_predefined_valid: 0,
            predefined_value: None,
            current_value: None,
        });
    }
    if status != NVAPI_OK {
        return Err(format_status(
            status,
            &format!("NvAPI_DRS_GetSetting(0x{id:08X})"),
        ));
    }
    Ok(SettingRead {
        status,
        existed: true,
        setting_type: read_u32(&buffer, SETTING_TYPE_OFFSET),
        location: read_u32(&buffer, SETTING_LOCATION_OFFSET),
        is_current_predefined: read_u32(&buffer, SETTING_CURRENT_PREDEFINED_OFFSET),
        is_predefined_valid: read_u32(&buffer, SETTING_PREDEFINED_VALID_OFFSET),
        predefined_value: Some(read_u32(&buffer, SETTING_PREDEFINED_VALUE_OFFSET)),
        current_value: Some(read_u32(&buffer, SETTING_CURRENT_VALUE_OFFSET)),
    })
}

fn read_required(
    session: &Session<'_>,
    profile: *mut c_void,
    id: u32,
    name: &str,
) -> Result<SettingRead, String> {
    let read = read_setting(session, profile, id)?;
    if !read.existed {
        return Err(format!(
            "{name} (0x{id:08X}) is not explicitly readable in the Jedi profile; status {}",
            read.status
        ));
    }
    Ok(read)
}

fn set_dword(
    session: &Session<'_>,
    profile: *mut c_void,
    id: u32,
    value: u32,
) -> Result<(), String> {
    let mut buffer = vec![0_u8; SETTING_STRUCT_SIZE];
    write_u32(&mut buffer, 0, version(SETTING_STRUCT_SIZE, 1));
    write_u32(&mut buffer, SETTING_ID_OFFSET, id);
    write_u32(&mut buffer, SETTING_TYPE_OFFSET, 0);
    write_u32(&mut buffer, SETTING_LOCATION_OFFSET, 0);
    write_u32(&mut buffer, SETTING_CURRENT_VALUE_OFFSET, value);
    let status =
        unsafe { (session.api.set_setting)(session.handle, profile, buffer.as_mut_ptr(), 0, 0) };
    check_status(status, &format!("NvAPI_DRS_SetSetting(raw, 0x{id:08X})"))?;
    check_status(
        unsafe { (session.api.save_settings)(session.handle) },
        "NvAPI_DRS_SaveSettings",
    )
}

fn delete_setting(session: &Session<'_>, profile: *mut c_void, id: u32) -> Result<(), String> {
    let status = unsafe { (session.api.delete_profile_setting)(session.handle, profile, id) };
    if status != NVAPI_OK && status != NVAPI_SETTING_NOT_FOUND {
        return Err(format_status(
            status,
            &format!("NvAPI_DRS_DeleteProfileSetting(0x{id:08X})"),
        ));
    }
    check_status(
        unsafe { (session.api.save_settings)(session.handle) },
        "NvAPI_DRS_SaveSettings",
    )
}

fn restore_one(
    session: &Session<'_>,
    target: &Target,
    snapshot: &Snapshot,
    id: u32,
) -> Result<(), String> {
    let saved = snapshot
        .settings
        .iter()
        .find(|setting| setting.id == id)
        .ok_or_else(|| format!("snapshot does not contain setting 0x{id:08X}"))?;
    if !saved.existed {
        // The snapshot was inherited/absent. Nothing in this spike created an
        // override for that setting, so a delete would be both unnecessary and
        // rejected by some drivers for newer RTX HDR IDs.
        return Ok(());
    }
    if saved.location == 0 && saved.setting_type == 0 {
        set_dword(
            session,
            target.profile,
            id,
            saved
                .current_value
                .ok_or_else(|| format!("snapshot value missing for 0x{id:08X}"))?,
        )?;
    } else {
        delete_setting(session, target.profile, id)?;
    }
    Ok(())
}

fn write_snapshot(path: &Path, snapshot: &Snapshot) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create snapshot directory: {error}"))?;
    }
    let data = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| format!("serialize snapshot: {error}"))?;
    fs::write(path, data).map_err(|error| format!("write snapshot {}: {error}", path.display()))
}

fn read_snapshot(path: &Path) -> Result<Snapshot, String> {
    let data =
        fs::read(path).map_err(|error| format!("read snapshot {}: {error}", path.display()))?;
    serde_json::from_slice(&data)
        .map_err(|error| format!("parse snapshot {}: {error}", path.display()))
}

fn default_snapshot_path() -> PathBuf {
    env::temp_dir().join("lumadeck-rtx-hdr-jedi-snapshot.json")
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn parse_i32_arg(args: &[String], index: usize, name: &str) -> Result<i32, String> {
    args.get(index)
        .ok_or_else(|| format!("missing {name}"))?
        .parse::<i32>()
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn fixed_utf16(value: &str) -> Result<Vec<u16>, String> {
    let mut result = vec![0_u16; 2_048];
    let encoded: Vec<u16> = value.encode_utf16().collect();
    if encoded.len() >= result.len() {
        return Err("NVAPI UnicodeString value is too long".to_string());
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

fn is_on(value: Option<u32>) -> bool {
    value == Some(1)
}

fn display_value_for(id: u32, value: Option<u32>) -> String {
    match value {
        None => "-".to_string(),
        Some(value) if id == RTX_HDR_CONTRAST_ID || id == RTX_HDR_SATURATION_ID => {
            format!("{} (display {:+})", value, value as i32 - 100)
        }
        Some(value) if id == RTX_HDR_ENABLE_ID => {
            format!("{} ({})", value, if value == 1 { "ON" } else { "OFF" })
        }
        Some(value) if id == RTX_HDR_PEAK_BRIGHTNESS_ID => format!("{} nits", value),
        Some(value) => value.to_string(),
    }
}

fn value_or_dash(value: Option<u32>) -> String {
    value.map_or_else(|| "-".to_string(), |value| value.to_string())
}

fn type_name(value: u32) -> &'static str {
    match value {
        0 => "DWORD",
        1 => "BINARY",
        2 => "STRING",
        3 => "WSTRING",
        4 => "QWORD",
        _ => "UNKNOWN",
    }
}

fn location_name(value: u32) -> &'static str {
    match value {
        0 => "CURRENT_PROFILE",
        1 => "GLOBAL_PROFILE",
        2 => "BASE_PROFILE",
        3 => "DEFAULT_PROFILE",
        _ => "UNKNOWN",
    }
}

fn check_status(status: i32, operation: &str) -> Result<(), String> {
    if status == NVAPI_OK {
        Ok(())
    } else {
        Err(format_status(status, operation))
    }
}

fn format_status(status: i32, operation: &str) -> String {
    let name = match status {
        NVAPI_OK => "NVAPI_OK",
        NVAPI_END_ENUMERATION => "NVAPI_END_ENUMERATION",
        NVAPI_SETTING_NOT_FOUND => "NVAPI_SETTING_NOT_FOUND",
        NVAPI_PROFILE_NOT_FOUND => "NVAPI_PROFILE_NOT_FOUND",
        NVAPI_APPLICATION_NOT_FOUND => "NVAPI_APPLICATION_NOT_FOUND",
        NVAPI_EXECUTABLE_NOT_FOUND => "NVAPI_EXECUTABLE_NOT_FOUND",
        NVAPI_ACCESS_DENIED => "NVAPI_ACCESS_DENIED",
        _ => "NVAPI_STATUS_UNKNOWN",
    };
    format!("{operation} returned {name} ({status})")
}

fn print_usage() {
    let mut usage = String::new();
    let _ = writeln!(
        usage,
        "LumaDeck RTX HDR DRS spike (raw NVAPI, per-game only)"
    );
    let _ = writeln!(usage, "  get");
    let _ = writeln!(
        usage,
        "  mappings                     # enumerate profiles containing Jedi's executable name"
    );
    let _ = writeln!(
        usage,
        "  set-contrast <display-value>  # e.g. 20 => stored 120"
    );
    let _ = writeln!(usage, "  restore-contrast");
    let _ = writeln!(usage, "  toggle-off");
    let _ = writeln!(usage, "  toggle-restore");
    let _ = writeln!(usage, "Options: --exe <absolute path> --snapshot <path>");
    print!("{usage}");
}
