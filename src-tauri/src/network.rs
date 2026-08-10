use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapter {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub adapter_type: String,
    pub state: String,
    pub connection_active: bool,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub mac: Option<String>,
    pub link_speed: Option<String>,
    pub wifi_interface_id: Option<String>,
    pub interface_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal_quality: u32,
    pub security: String,
    pub connected: bool,
    pub known: bool,
    pub interface_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    pub adapters: Vec<NetworkAdapter>,
    pub wifi_networks: Vec<WifiNetwork>,
    pub internet_state: String,
    pub active_connection_type: Option<String>,
    pub wifi_enabled: bool,
}

#[cfg(not(windows))]
pub fn get_network_state() -> Result<NetworkSnapshot, String> {
    Err("NETWORK_UNAVAILABLE".to_string())
}

#[cfg(not(windows))]
pub fn scan_wifi_networks() -> Result<NetworkSnapshot, String> {
    get_network_state()
}

#[cfg(not(windows))]
pub fn set_wifi_enabled(_adapter_id: String, _enabled: bool) -> Result<NetworkSnapshot, String> {
    get_network_state()
}

#[cfg(not(windows))]
pub fn set_network_adapter_enabled(
    _adapter_id: String,
    _enabled: bool,
) -> Result<NetworkSnapshot, String> {
    get_network_state()
}

#[cfg(not(windows))]
pub fn connect_wifi(
    _adapter_id: String,
    _ssid: String,
    _password: Option<String>,
) -> Result<NetworkSnapshot, String> {
    get_network_state()
}

#[cfg(not(windows))]
pub fn disconnect_wifi(_adapter_id: String) -> Result<NetworkSnapshot, String> {
    get_network_state()
}

#[cfg(not(windows))]
pub fn forget_wifi(_adapter_id: String, _ssid: String) -> Result<NetworkSnapshot, String> {
    get_network_state()
}

#[cfg(windows)]
mod windows_impl {
    use super::{NetworkAdapter, NetworkSnapshot, WifiNetwork};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::Deserialize;
    use std::{
        ffi::OsStr, os::windows::ffi::OsStrExt, process::Command, ptr, slice, thread,
        time::Duration,
    };
    use windows_sys::{
        core::GUID,
        Win32::{
            Foundation::{BOOL, HANDLE},
            NetworkManagement::WiFi::{
                dot11_BSS_type_infrastructure, wlan_connection_mode_profile, WlanCloseHandle,
                WlanConnect, WlanDeleteProfile, WlanEnumInterfaces, WlanFreeMemory,
                WlanGetAvailableNetworkList, WlanOpenHandle, WlanScan, WlanSetProfile,
                WLAN_AVAILABLE_NETWORK, WLAN_AVAILABLE_NETWORK_CONNECTED,
                WLAN_AVAILABLE_NETWORK_HAS_PROFILE, WLAN_CONNECTION_PARAMETERS,
                WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST, WLAN_PROFILE_USER,
            },
        },
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PowerShellAdapter {
        id: String,
        name: String,
        adapter_type: String,
        status: String,
        interface_index: u32,
        interface_guid: Option<String>,
        ipv4: Vec<String>,
        ipv6: Vec<String>,
        gateway: Vec<String>,
        dns: Vec<String>,
        mac: Option<String>,
        link_speed: Option<String>,
        connectivity: String,
    }

    pub fn get_network_state() -> Result<NetworkSnapshot, String> {
        let adapters = read_adapters()?;
        let wifi_networks = read_wifi_networks()?;
        Ok(build_snapshot(adapters, wifi_networks))
    }

    pub fn scan_wifi_networks() -> Result<NetworkSnapshot, String> {
        trigger_wifi_scan()?;
        thread::sleep(Duration::from_millis(650));
        get_network_state()
    }

    pub fn set_wifi_enabled(adapter_id: String, enabled: bool) -> Result<NetworkSnapshot, String> {
        set_network_adapter_enabled(adapter_id, enabled)
    }

    pub fn set_network_adapter_enabled(
        adapter_id: String,
        enabled: bool,
    ) -> Result<NetworkSnapshot, String> {
        let id = ps_literal(&adapter_id);
        let toggle = if enabled { "$true" } else { "$false" };
        let script = format!(
            "$ErrorActionPreference='Stop'; $a=Get-NetAdapter -IncludeHidden | Where-Object {{ $_.InterfaceGuid -eq '{id}' -or $_.ifIndex -eq '{id}' }} | Select-Object -First 1; if ($null -eq $a) {{ exit 44 }}; if ({toggle}) {{ Enable-NetAdapter -Name $a.Name -Confirm:$false }} else {{ Disable-NetAdapter -Name $a.Name -Confirm:$false }}"
        );
        run_elevated_powershell(&script)?;
        get_network_state()
    }

    pub fn connect_wifi(
        adapter_id: String,
        ssid: String,
        password: Option<String>,
    ) -> Result<NetworkSnapshot, String> {
        let interface_guid = parse_guid(&adapter_id)?;
        with_wlan_handle(|handle| {
            if let Some(secret) = password {
                let xml = profile_xml(&ssid, &secret);
                let profile = wide(&xml);
                let mut reason_code = 0u32;
                let result = unsafe {
                    WlanSetProfile(
                        handle,
                        &interface_guid,
                        WLAN_PROFILE_USER,
                        profile.as_ptr(),
                        ptr::null(),
                        1 as BOOL,
                        ptr::null(),
                        &mut reason_code,
                    )
                };
                if result != 0 {
                    return Err("WIFI_PROFILE_REJECTED".to_string());
                }
            }
            connect_profile(handle, &interface_guid, &ssid)
        })?;
        get_network_state()
    }

    pub fn disconnect_wifi(adapter_id: String) -> Result<NetworkSnapshot, String> {
        let interface_guid = parse_guid(&adapter_id)?;
        with_wlan_handle(|handle| {
            let result = unsafe {
                windows_sys::Win32::NetworkManagement::WiFi::WlanDisconnect(
                    handle,
                    &interface_guid,
                    ptr::null(),
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err("WIFI_DISCONNECT_FAILED".to_string())
            }
        })?;
        get_network_state()
    }

    pub fn forget_wifi(adapter_id: String, ssid: String) -> Result<NetworkSnapshot, String> {
        let interface_guid = parse_guid(&adapter_id)?;
        let profile = wide(&ssid);
        with_wlan_handle(|handle| {
            let result = unsafe {
                WlanDeleteProfile(handle, &interface_guid, profile.as_ptr(), ptr::null())
            };
            if result == 0 {
                Ok(())
            } else {
                Err("WIFI_FORGET_FAILED".to_string())
            }
        })?;
        get_network_state()
    }

    fn read_adapters() -> Result<Vec<NetworkAdapter>, String> {
        let script = r#"
$rows = @(Get-NetAdapter -IncludeHidden -ErrorAction Stop | Where-Object { $_.HardwareInterface -eq $true } | ForEach-Object {
  $i = $_.ifIndex
  $ip4 = @(Get-NetIPAddress -InterfaceIndex $i -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike '169.254.*' } | Select-Object -ExpandProperty IPAddress)
  $ip6 = @(Get-NetIPAddress -InterfaceIndex $i -AddressFamily IPv6 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike 'fe80::*' } | Select-Object -ExpandProperty IPAddress)
  $gw = @(Get-NetRoute -InterfaceIndex $i -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric | Select-Object -First 1 -ExpandProperty NextHop)
  $dns = @(Get-DnsClientServerAddress -InterfaceIndex $i -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ServerAddresses)
  $profile = Get-NetConnectionProfile -InterfaceIndex $i -ErrorAction SilentlyContinue | Select-Object -First 1
  $type = if ($_.Name -match 'vEthernet|virtual|hyper-v|loopback' -or $_.InterfaceDescription -match 'vEthernet|virtual|hyper-v|loopback') { 'other' } elseif ($_.Name -match 'wi-?fi|wireless' -or $_.InterfaceDescription -match 'wi-?fi|wireless|802.11') { 'wifi' } elseif ($_.Name -match 'ethernet|lan' -or $_.InterfaceDescription -match 'ethernet|gigabit|lan') { 'ethernet' } else { 'other' }
  $connectivity = if ($_.Status -eq 'Disabled') { 'disabled' } elseif ($null -eq $profile) { 'disconnected' } elseif ($profile.IPv4Connectivity -eq 'Internet' -or $profile.IPv6Connectivity -eq 'Internet') { 'connected' } elseif ($profile.IPv4Connectivity -ne 'None' -or $profile.IPv6Connectivity -ne 'None') { 'connected-no-internet' } else { 'disconnected' }
  [pscustomobject]@{ id = [string]$_.InterfaceGuid; name = [string]$_.Name; adapterType = $type; status = [string]$_.Status; interfaceIndex = [int]$i; interfaceGuid = [string]$_.InterfaceGuid; ipv4 = $ip4; ipv6 = $ip6; gateway = $gw; dns = $dns; mac = [string]$_.MacAddress; linkSpeed = [string]$_.LinkSpeed; connectivity = $connectivity }
})
@($rows) | ConvertTo-Json -Compress -Depth 4
"#;
        let raw = run_powershell(script)?;
        let rows: Vec<PowerShellAdapter> = parse_json_array(&raw)?;
        Ok(rows
            .into_iter()
            .map(|row| NetworkAdapter {
                id: row.id,
                name: row.name,
                adapter_type: row.adapter_type.clone(),
                state: if row.status.eq_ignore_ascii_case("disabled") {
                    "disabled".to_string()
                } else {
                    row.connectivity.clone()
                },
                connection_active: row.connectivity == "connected"
                    || row.connectivity == "connected-no-internet",
                ipv4: row.ipv4.first().cloned(),
                ipv6: row.ipv6.first().cloned(),
                gateway: row.gateway.first().cloned(),
                dns: row.dns,
                mac: non_empty(row.mac),
                link_speed: non_empty(row.link_speed),
                wifi_interface_id: (row.adapter_type == "wifi")
                    .then_some(row.interface_guid.unwrap_or_default()),
                interface_index: Some(row.interface_index),
            })
            .collect())
    }

    fn read_wifi_networks() -> Result<Vec<WifiNetwork>, String> {
        with_wlan_handle(|handle| {
            let interfaces = enum_interfaces(handle)?;
            let mut networks = Vec::new();
            for interface in interfaces {
                let mut list_ptr = ptr::null_mut();
                let result = unsafe {
                    WlanGetAvailableNetworkList(
                        handle,
                        &interface.InterfaceGuid,
                        0,
                        ptr::null(),
                        &mut list_ptr,
                    )
                };
                if result != 0 || list_ptr.is_null() {
                    continue;
                }
                let list = unsafe { &*list_ptr };
                let entries = unsafe {
                    slice::from_raw_parts(list.Network.as_ptr(), list.dwNumberOfItems as usize)
                };
                let interface_id = guid_to_string(&interface.InterfaceGuid);
                networks.extend(
                    entries
                        .iter()
                        .filter_map(|entry| wifi_network(entry, &interface_id)),
                );
                unsafe { WlanFreeMemory(list_ptr.cast()) };
            }
            Ok(networks)
        })
    }

    fn trigger_wifi_scan() -> Result<(), String> {
        with_wlan_handle(|handle| {
            for interface in enum_interfaces(handle)? {
                let result = unsafe {
                    WlanScan(
                        handle,
                        &interface.InterfaceGuid,
                        ptr::null(),
                        ptr::null(),
                        ptr::null(),
                    )
                };
                if result != 0 {
                    return Err("WIFI_SCAN_FAILED".to_string());
                }
            }
            Ok(())
        })
    }

    fn enum_interfaces(handle: HANDLE) -> Result<Vec<WLAN_INTERFACE_INFO>, String> {
        let mut list_ptr: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        let result = unsafe { WlanEnumInterfaces(handle, ptr::null(), &mut list_ptr) };
        if result != 0 || list_ptr.is_null() {
            return Err("WIFI_ADAPTER_UNAVAILABLE".to_string());
        }
        let list = unsafe { &*list_ptr };
        let interfaces = unsafe {
            slice::from_raw_parts(list.InterfaceInfo.as_ptr(), list.dwNumberOfItems as usize)
        }
        .to_vec();
        unsafe { WlanFreeMemory(list_ptr.cast()) };
        Ok(interfaces)
    }

    fn wifi_network(entry: &WLAN_AVAILABLE_NETWORK, interface_id: &str) -> Option<WifiNetwork> {
        let length = usize::try_from(entry.dot11Ssid.uSSIDLength).ok()?;
        if length == 0 || length > entry.dot11Ssid.ucSSID.len() {
            return None;
        }
        let ssid = String::from_utf8_lossy(&entry.dot11Ssid.ucSSID[..length]).to_string();
        if ssid.trim().is_empty() {
            return None;
        }
        Some(WifiNetwork {
            ssid,
            signal_quality: entry.wlanSignalQuality,
            security: if entry.bSecurityEnabled != 0 {
                "secured"
            } else {
                "open"
            }
            .to_string(),
            connected: entry.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0,
            known: entry.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE != 0,
            interface_id: interface_id.to_string(),
        })
    }

    fn connect_profile(handle: HANDLE, guid: &GUID, ssid: &str) -> Result<(), String> {
        let profile = wide(ssid);
        let parameters = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: profile.as_ptr(),
            pDot11Ssid: ptr::null_mut(),
            pDesiredBssidList: ptr::null_mut(),
            dot11BssType: dot11_BSS_type_infrastructure,
            dwFlags: 0,
        };
        let result = unsafe { WlanConnect(handle, guid, &parameters, ptr::null()) };
        if result == 0 {
            Ok(())
        } else {
            Err("WIFI_CONNECT_FAILED".to_string())
        }
    }

    fn with_wlan_handle<T>(
        callback: impl FnOnce(HANDLE) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut negotiated_version = 0u32;
        let mut handle: HANDLE = ptr::null_mut();
        let result =
            unsafe { WlanOpenHandle(2, ptr::null(), &mut negotiated_version, &mut handle) };
        if result != 0 || handle.is_null() {
            return Err("WIFI_SERVICE_UNAVAILABLE".to_string());
        }
        let result = callback(handle);
        unsafe { WlanCloseHandle(handle, ptr::null()) };
        result
    }

    fn build_snapshot(
        adapters: Vec<NetworkAdapter>,
        wifi_networks: Vec<WifiNetwork>,
    ) -> NetworkSnapshot {
        let active = adapters
            .iter()
            .find(|adapter| adapter.connection_active && adapter.adapter_type == "ethernet")
            .or_else(|| {
                adapters
                    .iter()
                    .find(|adapter| adapter.connection_active && adapter.adapter_type == "wifi")
            })
            .or_else(|| adapters.iter().find(|adapter| adapter.connection_active));
        let internet_state = if adapters.iter().any(|adapter| adapter.state == "connected") {
            "connected"
        } else if adapters
            .iter()
            .any(|adapter| adapter.state == "connected-no-internet")
        {
            "connected-no-internet"
        } else if adapters.iter().any(|adapter| adapter.state == "connecting") {
            "connecting"
        } else {
            "disconnected"
        };
        NetworkSnapshot {
            wifi_enabled: adapters
                .iter()
                .any(|adapter| adapter.adapter_type == "wifi" && adapter.state != "disabled"),
            active_connection_type: active.map(|adapter| adapter.adapter_type.clone()),
            adapters,
            wifi_networks,
            internet_state: internet_state.to_string(),
        }
    }

    fn run_powershell(script: &str) -> Result<String, String> {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
            .map_err(|_| "NETWORK_SERVICE_UNAVAILABLE".to_string())?;
        if !output.status.success() {
            return Err("NETWORK_OPERATION_FAILED".to_string());
        }
        String::from_utf8(output.stdout).map_err(|_| "NETWORK_RESPONSE_INVALID".to_string())
    }

    fn run_elevated_powershell(script: &str) -> Result<(), String> {
        let encoded_script = encode_powershell_command(script);
        let child_arguments = format!(
            "-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {encoded_script}"
        );
        let launcher = format!(
            "$p=Start-Process -FilePath 'powershell.exe' -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '{}'; exit $p.ExitCode",
            ps_literal(&child_arguments)
        );
        run_powershell(&launcher)
            .map(|_| ())
            .map_err(|error| match error.as_str() {
                "NETWORK_OPERATION_FAILED" => "NETWORK_OPERATION_REQUIRES_ADMIN".to_string(),
                _ => error,
            })
    }

    fn encode_powershell_command(script: &str) -> String {
        let bytes = script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<u8>>();
        STANDARD.encode(bytes)
    }

    fn parse_json_array<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<Vec<T>, String> {
        serde_json::from_str(raw.trim()).map_err(|_| "NETWORK_RESPONSE_INVALID".to_string())
    }

    fn non_empty(value: Option<String>) -> Option<String> {
        value.filter(|item| !item.trim().is_empty())
    }
    fn ps_literal(value: &str) -> String {
        value.replace('\'', "''")
    }
    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }
    fn profile_xml(ssid: &str, password: &str) -> String {
        let ssid = xml_escape(ssid);
        let password = xml_escape(password);
        format!("<WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\"><name>{ssid}</name><SSIDConfig><SSID><name>{ssid}</name></SSID></SSIDConfig><connectionType>ESS</connectionType><connectionMode>auto</connectionMode><MSM><security><authEncryption><authentication>WPA2PSK</authentication><encryption>AES</encryption><useOneX>false</useOneX></authEncryption><sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>{password}</keyMaterial></sharedKey></security></MSM></WLANProfile>")
    }
    fn xml_escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    fn parse_guid(value: &str) -> Result<GUID, String> {
        let trimmed = value.trim().trim_start_matches('{').trim_end_matches('}');
        let parts: Vec<&str> = trimmed.split('-').collect();
        if parts.len() != 5 || parts[3].len() != 4 || parts[4].len() != 12 {
            return Err("WIFI_ADAPTER_UNAVAILABLE".to_string());
        }
        let data1 = u32::from_str_radix(parts[0], 16)
            .map_err(|_| "WIFI_ADAPTER_UNAVAILABLE".to_string())?;
        let data2 = u16::from_str_radix(parts[1], 16)
            .map_err(|_| "WIFI_ADAPTER_UNAVAILABLE".to_string())?;
        let data3 = u16::from_str_radix(parts[2], 16)
            .map_err(|_| "WIFI_ADAPTER_UNAVAILABLE".to_string())?;
        let mut data4 = [0u8; 8];
        data4[0] = u8::from_str_radix(&parts[3][0..2], 16)
            .map_err(|_| "WIFI_ADAPTER_UNAVAILABLE".to_string())?;
        data4[1] = u8::from_str_radix(&parts[3][2..4], 16)
            .map_err(|_| "WIFI_ADAPTER_UNAVAILABLE".to_string())?;
        for index in 0..6 {
            data4[index + 2] = u8::from_str_radix(&parts[4][index * 2..index * 2 + 2], 16)
                .map_err(|_| "WIFI_ADAPTER_UNAVAILABLE".to_string())?;
        }
        Ok(GUID {
            data1,
            data2,
            data3,
            data4,
        })
    }
    fn guid_to_string(guid: &GUID) -> String {
        format!(
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            guid.data1,
            guid.data2,
            guid.data3,
            guid.data4[0],
            guid.data4[1],
            guid.data4[2],
            guid.data4[3],
            guid.data4[4],
            guid.data4[5],
            guid.data4[6],
            guid.data4[7]
        )
    }
}

#[cfg(windows)]
pub use windows_impl::{
    connect_wifi, disconnect_wifi, forget_wifi, get_network_state, scan_wifi_networks,
    set_network_adapter_enabled, set_wifi_enabled,
};
