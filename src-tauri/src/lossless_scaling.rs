use crate::frame_generation::{
    FrameGenerationProvider, FrameGenerationSync, LosslessScalingStatus,
};
use crate::settings::FrameGenerationProfile;
use quick_xml::events::{
    BytesCData, BytesDecl, BytesEnd, BytesPI, BytesRef, BytesStart, BytesText, Event,
};
use quick_xml::{Reader, Writer};
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

const SETTINGS_FILE: &str = "Settings.xml";
const BACKUP_FILE: &str = "Settings.xml.lumadeck-backup";
const TEMP_FILE: &str = "Settings.xml.tmp";

static RESTART_REQUIRED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default)]
pub struct LosslessScalingProvider;

impl FrameGenerationProvider for LosslessScalingProvider {
    fn synchronize_if_needed(
        &self,
        profile: &FrameGenerationProfile,
    ) -> Result<FrameGenerationSync, String> {
        let settings =
            settings_path().ok_or_else(|| "LOSSLESS_SCALING_NOT_INSTALLED".to_string())?;
        if !settings.is_file() {
            return Err("LOSSLESS_SCALING_NOT_INSTALLED".to_string());
        }
        let original =
            fs::read(&settings).map_err(|_| "LOSSLESS_SCALING_SETTINGS_READ".to_string())?;
        let mut document = parse_document(&original)
            .map_err(|_| "LOSSLESS_SCALING_SETTINGS_INVALID".to_string())?;
        let profiles = document
            .find_game_profiles_mut()
            .ok_or_else(|| "LOSSLESS_SCALING_GAME_PROFILES_MISSING".to_string())?;
        let target_index =
            profiles
                .children
                .iter()
                .enumerate()
                .find_map(|(index, node)| match node {
                    XmlNode::Element(element)
                        if normalized_path(element.child_text("Path").as_deref())
                            == normalized_path(profile.target_executable.as_deref()) =>
                    {
                        Some(index)
                    }
                    _ => None,
                });

        let changed = if let Some(index) = target_index {
            let XmlNode::Element(element) = &mut profiles.children[index] else {
                return Err("LOSSLESS_SCALING_PROFILE_INVALID".to_string());
            };
            let changed = !managed_fields_match(element, profile);
            if changed {
                update_managed_fields(element, profile, false);
            }
            changed
        } else {
            let default = profiles
                .children
                .iter()
                .find_map(|node| match node {
                    XmlNode::Element(element)
                        if element
                            .child_text("Title")
                            .is_some_and(|title| title.eq_ignore_ascii_case("Default")) =>
                    {
                        Some(element.clone())
                    }
                    _ => None,
                })
                .ok_or_else(|| "LOSSLESS_SCALING_DEFAULT_PROFILE_MISSING".to_string())?;
            let mut created = default;
            set_child_text(&mut created, "Title", "LumaDeck");
            set_child_text(
                &mut created,
                "Path",
                profile.target_executable.as_deref().unwrap_or_default(),
            );
            update_managed_fields(&mut created, profile, true);
            profiles.children.push(XmlNode::Element(created));
            true
        };

        if !changed {
            return Ok(FrameGenerationSync {
                restart_required: restart_required_state(),
            });
        }

        let serialized = serialize_document(&document)
            .map_err(|_| "LOSSLESS_SCALING_SETTINGS_WRITE".to_string())?;
        parse_document(&serialized)
            .map_err(|_| "LOSSLESS_SCALING_SETTINGS_INVALID_TEMP".to_string())?;
        create_backup_once(&settings, &original)?;
        atomic_replace(&settings, &serialized)?;
        let restart_required = is_lossless_scaling_running();
        RESTART_REQUIRED.store(restart_required, Ordering::SeqCst);
        Ok(FrameGenerationSync { restart_required })
    }

    fn status(&self) -> LosslessScalingStatus {
        let settings = settings_path();
        let application = find_application_path();
        let settings_exists = settings.as_ref().is_some_and(|path| path.is_file());
        let settings_status = match settings.as_ref() {
            None => "missing".to_string(),
            Some(path) if !path.is_file() => "missing".to_string(),
            Some(path) => match fs::read(path)
                .ok()
                .and_then(|bytes| parse_document(&bytes).ok())
            {
                Some(_) => "valid".to_string(),
                None => "invalid".to_string(),
            },
        };
        let installed = application.is_some() || settings_exists;
        let process_running = is_lossless_scaling_running();
        LosslessScalingStatus {
            status: provider_status(installed, settings_status == "valid", process_running)
                .to_string(),
            version: application
                .as_deref()
                .and_then(application_version)
                .unwrap_or_else(|| "Unknown".to_string()),
            installation_path: application
                .as_ref()
                .and_then(|path| path.parent())
                .map(|path| path.display().to_string()),
            settings_path: settings
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            settings_status,
            application_running: process_running,
            restart_required: restart_required_state(),
        }
    }

    fn start_background(&self) -> Result<(), String> {
        if !should_start_background(is_lossless_scaling_running()) {
            return Ok(());
        }
        let application =
            find_application_path().ok_or_else(|| "LOSSLESS_SCALING_NOT_INSTALLED".to_string())?;
        spawn_application(&application, true)
    }

    fn open_application(&self) -> Result<(), String> {
        // Lossless Scaling 3.2.2 does not expose a reliable tray-restore API.
        // Do not spawn a second instance when the existing process is already running.
        if is_lossless_scaling_running() {
            return Ok(());
        }
        let application =
            find_application_path().ok_or_else(|| "LOSSLESS_SCALING_NOT_INSTALLED".to_string())?;
        spawn_application(&application, false)
    }

    fn restart_background(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            for pid in lossless_scaling_process_ids() {
                request_normal_close(pid);
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while is_lossless_scaling_running() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if is_lossless_scaling_running() {
                return Err("LOSSLESS_SCALING_CLOSE_TIMEOUT".to_string());
            }
        }
        RESTART_REQUIRED.store(false, Ordering::SeqCst);
        self.start_background()
    }

    fn restore_backup(&self) -> Result<(), String> {
        let settings =
            settings_path().ok_or_else(|| "LOSSLESS_SCALING_NOT_INSTALLED".to_string())?;
        let backup = settings.with_file_name(BACKUP_FILE);
        let contents =
            fs::read(&backup).map_err(|_| "LOSSLESS_SCALING_BACKUP_MISSING".to_string())?;
        parse_document(&contents).map_err(|_| "LOSSLESS_SCALING_BACKUP_INVALID".to_string())?;
        atomic_replace(&settings, &contents)
    }
}

pub fn is_lossless_scaling_running() -> bool {
    #[cfg(windows)]
    {
        use std::mem;
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut entry: PROCESSENTRY32W = mem::zeroed();
            entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = false;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let length = entry
                        .szExeFile
                        .iter()
                        .position(|value| *value == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
                    if normalize_name(&name) == "losslessscaling" {
                        found = true;
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
            found
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
fn lossless_scaling_process_ids() -> Vec<u32> {
    use std::mem;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ids = Vec::new();
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let length = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
                if normalize_name(&name) == "losslessscaling" {
                    ids.push(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        ids
    }
}

#[cfg(windows)]
fn request_normal_close(pid: u32) {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE, WNDENUMPROC,
    };
    unsafe extern "system" fn callback(hwnd: *mut core::ffi::c_void, parameter: LPARAM) -> BOOL {
        let context = &mut *(parameter as *mut (u32, bool));
        let mut window_pid = 0;
        GetWindowThreadProcessId(hwnd, &mut window_pid);
        if window_pid == context.0 {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            context.1 = true;
            return 0;
        }
        1
    }
    let mut context = (pid, false);
    let callback: WNDENUMPROC = Some(callback);
    unsafe {
        EnumWindows(callback, &mut context as *mut (u32, bool) as LPARAM);
    }
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Lossless Scaling").join(SETTINGS_FILE))
}

fn find_application_path() -> Option<PathBuf> {
    application_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

fn application_arguments(background: bool) -> &'static [&'static str] {
    if background {
        &["-StartMinimized"]
    } else {
        &[]
    }
}

fn should_start_background(application_running: bool) -> bool {
    !application_running
}

fn provider_status(installed: bool, settings_valid: bool, process_running: bool) -> &'static str {
    if !installed {
        "NotInstalled"
    } else if !settings_valid {
        "ConfigurationInvalid"
    } else if !process_running {
        "NotRunning"
    } else {
        "Ready"
    }
}

fn spawn_application(application: &Path, background: bool) -> Result<(), String> {
    debug_assert!(background || application_arguments(background).is_empty());
    Command::new(application)
        .current_dir(application.parent().unwrap_or_else(|| Path::new(".")))
        .args(application_arguments(background))
        .spawn()
        .map(|_| ())
        .map_err(|_| "LOSSLESS_SCALING_START_FAILED".to_string())
}

fn application_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let root = PathBuf::from(local);
        candidates.push(root.join("Lossless Scaling/LosslessScaling.exe"));
        candidates.push(root.join("Lossless Scaling/Lossless Scaling.exe"));
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(variable) {
            let root = PathBuf::from(root);
            candidates.push(root.join("Lossless Scaling/LosslessScaling.exe"));
            candidates.push(root.join("Lossless Scaling/Lossless Scaling.exe"));
            candidates
                .push(root.join("Steam/steamapps/common/Lossless Scaling/LosslessScaling.exe"));
            candidates
                .push(root.join("Steam/steamapps/common/Lossless Scaling/Lossless Scaling.exe"));
        }
    }
    candidates
}

fn application_version(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-Item -LiteralPath $args[0]).VersionInfo.ProductVersion",
                path.to_string_lossy().as_ref(),
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!version.is_empty()).then_some(version)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

fn create_backup_once(settings: &Path, contents: &[u8]) -> Result<(), String> {
    let backup = settings.with_file_name(BACKUP_FILE);
    if backup.exists() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup)
        .map_err(|_| "LOSSLESS_SCALING_BACKUP_WRITE".to_string())?;
    file.write_all(contents)
        .map_err(|_| "LOSSLESS_SCALING_BACKUP_WRITE".to_string())?;
    file.sync_all()
        .map_err(|_| "LOSSLESS_SCALING_BACKUP_WRITE".to_string())
}

fn atomic_replace(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temporary = path.with_file_name(TEMP_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary)
        .map_err(|_| "LOSSLESS_SCALING_TEMP_WRITE".to_string())?;
    file.write_all(contents)
        .map_err(|_| "LOSSLESS_SCALING_TEMP_WRITE".to_string())?;
    file.sync_all()
        .map_err(|_| "LOSSLESS_SCALING_TEMP_WRITE".to_string())?;
    drop(file);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let source: Vec<u16> = temporary
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let target: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            return Err("LOSSLESS_SCALING_ATOMIC_REPLACE_FAILED".to_string());
        }
    }
    #[cfg(not(windows))]
    fs::rename(&temporary, path)
        .map_err(|_| "LOSSLESS_SCALING_ATOMIC_REPLACE_FAILED".to_string())?;
    Ok(())
}

fn normalized_path(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn normalize_name(value: &str) -> String {
    let value = value.rsplit_once('.').map_or(value, |(stem, _)| stem);
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn restart_required_state() -> bool {
    if !is_lossless_scaling_running() {
        RESTART_REQUIRED.store(false, Ordering::SeqCst);
    }
    RESTART_REQUIRED.load(Ordering::SeqCst)
}

fn managed_fields_match(element: &XmlElement, profile: &FrameGenerationProfile) -> bool {
    element.child_text("FrameGeneration").as_deref()
        == Some(if profile.enabled { "LSFG3" } else { "Off" })
        && element.child_text("LSFG3Mode1").as_deref() == Some(profile.mode.as_str())
        && element.child_text("LSFG3Multiplier").as_deref()
            == Some(profile.multiplier.to_string().as_str())
        && element.child_text("AutoScale").as_deref()
            == Some(if profile.auto_scale { "true" } else { "false" })
        && element.child_text("AutoScaleDelay").as_deref()
            == Some(profile.auto_scale_delay.to_string().as_str())
}

fn update_managed_fields(element: &mut XmlElement, profile: &FrameGenerationProfile, is_new: bool) {
    set_child_text(
        element,
        "FrameGeneration",
        if profile.enabled { "LSFG3" } else { "Off" },
    );
    set_child_text(element, "LSFG3Mode1", &profile.mode);
    set_child_text(element, "LSFG3Multiplier", &profile.multiplier.to_string());
    set_child_text(
        element,
        "AutoScale",
        if profile.auto_scale { "true" } else { "false" },
    );
    set_child_text(
        element,
        "AutoScaleDelay",
        &profile.auto_scale_delay.to_string(),
    );
    if is_new {
        set_child_text(element, "ScalingType", "Off");
        set_child_text(element, "CaptureApi", "DXGI");
    }
}

fn set_child_text(element: &mut XmlElement, name: &str, value: &str) {
    if let Some(child) = element.children.iter_mut().find_map(|node| match node {
        XmlNode::Element(child) if child.name == name => Some(child),
        _ => None,
    }) {
        child.children = vec![XmlNode::Text(value.to_string())];
        child.empty = false;
        return;
    }
    element.children.push(XmlNode::Element(XmlElement {
        name: name.to_string(),
        attributes: Vec::new(),
        children: vec![XmlNode::Text(value.to_string())],
        empty: false,
    }));
}

#[derive(Debug, Clone)]
struct XmlDocument {
    nodes: Vec<XmlNode>,
}

#[derive(Debug, Clone)]
enum XmlNode {
    Element(XmlElement),
    Text(String),
    CData(String),
    Comment(String),
    Declaration(String),
    DocType(String),
    ProcessingInstruction(String),
    GeneralReference(String),
}

#[derive(Debug, Clone)]
struct XmlElement {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<XmlNode>,
    empty: bool,
}

impl XmlElement {
    fn child_text(&self, name: &str) -> Option<String> {
        self.children.iter().find_map(|node| match node {
            XmlNode::Element(element) if element.name == name => Some(element.text_content()),
            _ => None,
        })
    }

    fn text_content(&self) -> String {
        self.children
            .iter()
            .filter_map(|node| match node {
                XmlNode::Text(value) | XmlNode::CData(value) => Some(value.as_str()),
                _ => None,
            })
            .collect()
    }
}

impl XmlDocument {
    fn find_game_profiles_mut(&mut self) -> Option<&mut XmlElement> {
        find_element_mut(&mut self.nodes, "GameProfiles")
    }
}

fn find_element_mut<'a>(nodes: &'a mut [XmlNode], name: &str) -> Option<&'a mut XmlElement> {
    for node in nodes {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if element.name == name {
            return Some(element);
        }
        if let Some(found) = find_element_mut(&mut element.children, name) {
            return Some(found);
        }
    }
    None
}

fn parse_document(bytes: &[u8]) -> Result<XmlDocument, String> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut roots = Vec::new();
    let mut stack = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| error.to_string())?
        {
            Event::Start(value) => stack.push(element_from_start(&value, reader.decoder())?),
            Event::Empty(value) => append_node(
                &mut roots,
                &mut stack,
                XmlNode::Element(element_from_start(&value, reader.decoder())?),
            ),
            Event::End(value) => {
                let element = stack.pop().ok_or_else(|| "unbalanced XML".to_string())?;
                if element.name.as_bytes() != value.name().as_ref() {
                    return Err("unbalanced XML".to_string());
                }
                append_node(&mut roots, &mut stack, XmlNode::Element(element));
            }
            Event::Text(value) => append_node(
                &mut roots,
                &mut stack,
                XmlNode::Text(
                    value
                        .decode()
                        .map_err(|error| error.to_string())?
                        .into_owned(),
                ),
            ),
            Event::CData(value) => append_node(
                &mut roots,
                &mut stack,
                XmlNode::CData(String::from_utf8_lossy(value.as_ref()).into_owned()),
            ),
            Event::Comment(value) => append_node(
                &mut roots,
                &mut stack,
                XmlNode::Comment(String::from_utf8_lossy(value.as_ref()).into_owned()),
            ),
            Event::Decl(value) => roots.push(XmlNode::Declaration(
                String::from_utf8_lossy(value.as_ref()).into_owned(),
            )),
            Event::DocType(value) => roots.push(XmlNode::DocType(
                String::from_utf8_lossy(value.as_ref()).into_owned(),
            )),
            Event::PI(value) => roots.push(XmlNode::ProcessingInstruction(
                String::from_utf8_lossy(value.as_ref()).into_owned(),
            )),
            Event::GeneralRef(value) => append_node(
                &mut roots,
                &mut stack,
                XmlNode::GeneralReference(String::from_utf8_lossy(value.as_ref()).into_owned()),
            ),
            Event::Eof => break,
        }
        buffer.clear();
    }
    if !stack.is_empty() {
        return Err("unclosed XML element".to_string());
    }
    Ok(XmlDocument { nodes: roots })
}

fn element_from_start(
    value: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> Result<XmlElement, String> {
    let mut attributes = Vec::new();
    for attribute in value.attributes().with_checks(false) {
        let attribute = attribute.map_err(|error| error.to_string())?;
        attributes.push((
            String::from_utf8_lossy(attribute.key.as_ref()).into_owned(),
            attribute
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
                .map_err(|error| error.to_string())?
                .into_owned(),
        ));
    }
    Ok(XmlElement {
        name: String::from_utf8_lossy(value.name().as_ref()).into_owned(),
        attributes,
        children: Vec::new(),
        empty: false,
    })
}

fn append_node(roots: &mut Vec<XmlNode>, stack: &mut Vec<XmlElement>, node: XmlNode) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}

fn serialize_document(document: &XmlDocument) -> Result<Vec<u8>, String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    for node in &document.nodes {
        write_node(&mut writer, node)?;
    }
    Ok(writer.into_inner().into_inner())
}

fn write_node(writer: &mut Writer<Cursor<Vec<u8>>>, node: &XmlNode) -> Result<(), String> {
    match node {
        XmlNode::Element(element) => {
            let mut start = BytesStart::new(element.name.as_str());
            for (name, value) in &element.attributes {
                start.push_attribute((name.as_str(), value.as_str()));
            }
            if element.empty && element.children.is_empty() {
                writer
                    .write_event(Event::Empty(start))
                    .map_err(|error| error.to_string())?;
            } else {
                writer
                    .write_event(Event::Start(start))
                    .map_err(|error| error.to_string())?;
                for child in &element.children {
                    write_node(writer, child)?;
                }
                writer
                    .write_event(Event::End(BytesEnd::new(element.name.as_str())))
                    .map_err(|error| error.to_string())?;
            }
        }
        XmlNode::Text(value) => writer
            .write_event(Event::Text(BytesText::new(value)))
            .map_err(|error| error.to_string())?,
        XmlNode::CData(value) => writer
            .write_event(Event::CData(BytesCData::new(value)))
            .map_err(|error| error.to_string())?,
        XmlNode::Comment(value) => writer
            .write_event(Event::Comment(BytesText::new(value)))
            .map_err(|error| error.to_string())?,
        XmlNode::Declaration(value) => {
            let _ = value;
            writer
                .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
                .map_err(|error| error.to_string())?;
        }
        XmlNode::DocType(value) => writer
            .write_event(Event::DocType(BytesText::new(value)))
            .map_err(|error| error.to_string())?,
        XmlNode::ProcessingInstruction(value) => writer
            .write_event(Event::PI(BytesPI::new(value)))
            .map_err(|error| error.to_string())?,
        XmlNode::GeneralReference(value) => writer
            .write_event(Event::GeneralRef(BytesRef::new(value)))
            .map_err(|error| error.to_string())?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        application_arguments, managed_fields_match, normalize_name, parse_document,
        provider_status, serialize_document, should_start_background, update_managed_fields,
        XmlNode,
    };
    use crate::settings::FrameGenerationProfile;

    #[test]
    fn preserves_unknown_nodes_and_updates_only_managed_fields() {
        let input = br#"<?xml version="1.0"?><Settings><GameProfiles><Profile><Title>Default</Title><Path>Default</Path><Unknown><Value>keep</Value></Unknown><FrameGeneration>Off</FrameGeneration><LSFG3Mode1>FIXED</LSFG3Mode1><LSFG3Multiplier>2</LSFG3Multiplier><AutoScale>false</AutoScale><AutoScaleDelay>0</AutoScaleDelay><QueueTarget>1</QueueTarget></Profile></GameProfiles></Settings>"#;
        let mut document = parse_document(input).expect("valid XML");
        let profiles = document.find_game_profiles_mut().expect("GameProfiles");
        let XmlNode::Element(profile) = &mut profiles.children[0] else {
            panic!("profile")
        };
        let frame_profile = FrameGenerationProfile {
            game_id: "game".to_string(),
            provider: "lossless-scaling".to_string(),
            enabled: true,
            mode: "FIXED".to_string(),
            multiplier: 2,
            auto_scale: true,
            auto_scale_delay: 0,
            target_executable: Some("C:\\Games\\game.exe".to_string()),
            updated_at: None,
            restart_required: false,
        };
        assert!(!managed_fields_match(profile, &frame_profile));
        update_managed_fields(profile, &frame_profile, false);
        let output =
            String::from_utf8(serialize_document(&document).expect("serialize")).expect("UTF-8");
        assert!(output.contains("<Unknown><Value>keep</Value></Unknown>"));
        assert!(output.contains("<QueueTarget>1</QueueTarget>"));
        assert!(output.contains("<FrameGeneration>LSFG3</FrameGeneration>"));
    }

    #[test]
    fn normalizes_process_names_with_executable_extension() {
        assert_eq!(normalize_name("LosslessScaling.exe"), "losslessscaling");
        assert_eq!(normalize_name("LosslessScaling"), "losslessscaling");
    }

    #[test]
    fn background_start_uses_start_minimized_and_never_relaunches_existing_process() {
        assert_eq!(application_arguments(true), ["-StartMinimized"]);
        assert!(application_arguments(false).is_empty());
        assert!(should_start_background(false));
        assert!(!should_start_background(true));
    }

    #[test]
    fn ready_requires_installation_valid_settings_and_running_process() {
        assert_eq!(provider_status(true, true, true), "Ready");
        assert_eq!(provider_status(true, true, false), "NotRunning");
        assert_eq!(provider_status(true, false, true), "ConfigurationInvalid");
        assert_eq!(provider_status(false, true, true), "NotInstalled");
    }

    #[test]
    fn rejects_malformed_xml() {
        assert!(parse_document(b"<Settings>").is_err());
    }
}
