use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_TELEMETRY_EVENTS: usize = 256;
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothTelemetryEvent {
    pub timestamp: String,
    pub level: String,
    pub event: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothAdapter {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub available: bool,
    pub discoverable: bool,
    pub hardware_present: bool,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothDevice {
    pub id: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
    pub connectable: bool,
    pub signal_strength: Option<i16>,
    pub device_class: String,
    pub battery_level: Option<u8>,
    pub last_seen: Option<String>,
    pub pairing_state: String,
    pub connection_state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothSnapshot {
    pub adapters: Vec<BluetoothAdapter>,
    pub devices: Vec<BluetoothDevice>,
    pub discovery_active: bool,
    pub available: bool,
    pub enabled: bool,
}

pub struct BluetoothService {
    telemetry: Arc<Mutex<VecDeque<BluetoothTelemetryEvent>>>,
    log_directory: Arc<Mutex<Option<PathBuf>>>,
    pairing_lock: Mutex<()>,
    pairing_sequence: AtomicU64,
    #[cfg(windows)]
    discovery: Mutex<Option<std::sync::Arc<Mutex<windows_impl::DiscoveryState>>>>,
    #[cfg(windows)]
    pairing_state: Mutex<windows_impl::PairingLifecycleState>,
}

impl Default for BluetoothService {
    fn default() -> Self {
        Self {
            telemetry: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_TELEMETRY_EVENTS))),
            log_directory: Arc::new(Mutex::new(None)),
            pairing_lock: Mutex::new(()),
            pairing_sequence: AtomicU64::new(0),
            #[cfg(windows)]
            discovery: Mutex::new(None),
            #[cfg(windows)]
            pairing_state: Mutex::new(windows_impl::PairingLifecycleState::Idle),
        }
    }
}

impl BluetoothService {
    pub fn configure_logging(&self, log_directory: PathBuf) {
        if let Ok(mut current) = self.log_directory.lock() {
            *current = Some(log_directory);
        }
    }

    pub fn log(&self, level: &str, event: &str, details: impl Into<String>) {
        self.log_sink().log(level, event, details);
    }

    fn log_sink(&self) -> BluetoothLogSink {
        BluetoothLogSink {
            telemetry: Arc::clone(&self.telemetry),
            log_directory: Arc::clone(&self.log_directory),
        }
    }
}

#[derive(Clone)]
struct BluetoothLogSink {
    telemetry: Arc<Mutex<VecDeque<BluetoothTelemetryEvent>>>,
    log_directory: Arc<Mutex<Option<PathBuf>>>,
}

impl BluetoothLogSink {
    fn log(&self, level: &str, event: &str, details: impl Into<String>) {
        let telemetry = BluetoothTelemetryEvent {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis().to_string())
                .unwrap_or_else(|_| "0".to_string()),
            level: level.to_string(),
            event: event.to_string(),
            details: details.into(),
        };
        if let Ok(mut events) = self.telemetry.lock() {
            events.push_back(telemetry.clone());
            while events.len() > MAX_TELEMETRY_EVENTS {
                events.pop_front();
            }
        }
        let line = format!(
            "[bluetooth] timestamp={} level={} event={} details={}\n",
            telemetry.timestamp, telemetry.level, telemetry.event, telemetry.details
        );
        #[cfg(debug_assertions)]
        eprint!("{line}");
        let log_directory = self
            .log_directory
            .lock()
            .ok()
            .and_then(|directory| directory.clone());
        let Some(log_directory) = log_directory else {
            return;
        };
        if fs::create_dir_all(&log_directory).is_err() {
            return;
        }
        let log_path = log_directory.join("bluetooth-runtime.log");
        if fs::metadata(&log_path)
            .map(|metadata| metadata.len() >= MAX_LOG_BYTES)
            .unwrap_or(false)
        {
            let backup_path = log_directory.join("bluetooth-runtime.log.1");
            let _ = fs::remove_file(&backup_path);
            let _ = fs::rename(&log_path, backup_path);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

impl BluetoothService {
    pub fn diagnostics(&self) -> Result<Vec<BluetoothTelemetryEvent>, String> {
        let events = self
            .telemetry
            .lock()
            .map_err(|_| "BLUETOOTH_DIAGNOSTICS_UNAVAILABLE".to_string())?;
        Ok(events.iter().cloned().collect())
    }
}

#[cfg(not(windows))]
pub fn get_bluetooth_state(service: &BluetoothService) -> Result<BluetoothSnapshot, String> {
    service.log("error", "state.unavailable", "platform=non-windows");
    Err("BLUETOOTH_UNAVAILABLE".to_string())
}

#[cfg(not(windows))]
pub fn set_bluetooth_enabled(
    service: &BluetoothService,
    enabled: bool,
) -> Result<BluetoothSnapshot, String> {
    service.log("info", "radio.toggle.request", format!("enabled={enabled}"));
    get_bluetooth_state(service)
}

#[cfg(not(windows))]
pub fn start_bluetooth_discovery(service: &BluetoothService) -> Result<BluetoothSnapshot, String> {
    service.log("info", "discovery.start.request", "platform=non-windows");
    get_bluetooth_state(service)
}

#[cfg(not(windows))]
pub fn stop_bluetooth_discovery(service: &BluetoothService) -> Result<BluetoothSnapshot, String> {
    service.log("info", "discovery.stop.request", "platform=non-windows");
    get_bluetooth_state(service)
}

#[cfg(not(windows))]
pub fn pair_bluetooth_device(
    service: &BluetoothService,
    _device_id: String,
) -> Result<BluetoothSnapshot, String> {
    service.log("info", "pair.request", "platform=non-windows");
    get_bluetooth_state(service)
}

#[cfg(not(windows))]
pub fn unpair_bluetooth_device(
    service: &BluetoothService,
    _device_id: String,
) -> Result<BluetoothSnapshot, String> {
    service.log("info", "unpair.request", "platform=non-windows");
    get_bluetooth_state(service)
}

#[cfg(windows)]
mod windows_impl {
    use super::{BluetoothAdapter, BluetoothDevice, BluetoothService, BluetoothSnapshot};
    use std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex, TryLockError,
        },
        thread,
        time::{Duration, Instant},
    };
    use windows::{
        core::{Interface, Ref, HSTRING},
        Devices::{
            Bluetooth::{
                BluetoothConnectionStatus, BluetoothDevice as WinBluetoothDevice,
                BluetoothLEDevice, BluetoothMajorClass, BluetoothMinorClass,
            },
            Enumeration::{
                DeviceInformation, DeviceInformationCustomPairing, DeviceInformationKind,
                DevicePairingKinds, DevicePairingRequestedEventArgs, DevicePairingResultStatus,
                DeviceUnpairingResultStatus, DeviceWatcher, DeviceWatcherStatus,
            },
            Radios::{Radio, RadioKind, RadioState},
        },
        Foundation::{IPropertyValue, TypedEventHandler},
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_SERVICE_ALREADY_RUNNING,
            ERROR_SERVICE_NOT_ACTIVE,
        },
        System::Services::{
            CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx,
            StartServiceW, SC_HANDLE, SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO,
            SERVICES_ACTIVE_DATABASE, SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
            SERVICE_START, SERVICE_STATUS, SERVICE_STATUS_PROCESS, SERVICE_STOP, SERVICE_STOPPED,
        },
        System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE},
        UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
    };

    pub(super) struct DiscoveryState {
        pub active: bool,
        pub devices: BTreeMap<String, BluetoothDevice>,
        pub watchers: Vec<DeviceWatcher>,
    }

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub(super) enum PairingLifecycleState {
        Idle,
        Discovering,
        PreparingPair,
        Pairing,
        WaitingForInteraction,
        RetryWait,
        RecoveringWindowsAssociation,
        Cancelling,
        Completed,
    }

    impl PairingLifecycleState {
        fn name(self) -> &'static str {
            match self {
                Self::Idle => "Idle",
                Self::Discovering => "Discovering",
                Self::PreparingPair => "PreparingPair",
                Self::Pairing => "Pairing",
                Self::WaitingForInteraction => "WaitingForInteraction",
                Self::RetryWait => "RetryWait",
                Self::RecoveringWindowsAssociation => "RecoveringWindowsAssociation",
                Self::Cancelling => "Cancelling",
                Self::Completed => "Completed",
            }
        }
    }

    fn set_pairing_state(
        service: &BluetoothService,
        attempt_id: &str,
        state: PairingLifecycleState,
    ) {
        if let Ok(mut current) = service.pairing_state.lock() {
            *current = state;
        }
        service.log(
            "info",
            "pair.state",
            format!("attempt_id={attempt_id} state={}", state.name()),
        );
    }

    struct PairingAttemptGuard<'a> {
        service: &'a BluetoothService,
        attempt_id: String,
        started_at: Instant,
        success: bool,
    }

    impl PairingAttemptGuard<'_> {
        fn mark_success(&mut self) {
            self.success = true;
        }
    }

    impl Drop for PairingAttemptGuard<'_> {
        fn drop(&mut self) {
            set_pairing_state(
                self.service,
                &self.attempt_id,
                PairingLifecycleState::Completed,
            );
            self.service.log(
                if self.success { "info" } else { "error" },
                "pair.attempt.end",
                format!(
                    "attempt_id={} success={} attempt_duration_ms={}",
                    self.attempt_id,
                    self.success,
                    self.started_at.elapsed().as_millis()
                ),
            );
            set_pairing_state(self.service, &self.attempt_id, PairingLifecycleState::Idle);
        }
    }

    enum PairAttemptResult {
        AlreadyPaired,
        Status(DevicePairingResultStatus),
    }

    pub fn get_bluetooth_state(service: &BluetoothService) -> Result<BluetoothSnapshot, String> {
        let (adapters, available, enabled) = read_adapters()?;
        let mut devices = read_paired_devices()?;
        let discovery = service
            .discovery
            .lock()
            .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?;
        let discovery_active = discovery
            .as_ref()
            .map(|state| state.lock().map(|value| value.active).unwrap_or(false))
            .unwrap_or(false);
        if let Some(state) = discovery.as_ref() {
            if let Ok(state) = state.lock() {
                merge_devices(&mut devices, state.devices.values().cloned());
            }
        }
        drop(discovery);
        devices.sort_by(|left, right| {
            right
                .connected
                .cmp(&left.connected)
                .then_with(|| right.paired.cmp(&left.paired))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(BluetoothSnapshot {
            adapters,
            devices,
            discovery_active,
            available,
            enabled,
        })
    }

    pub fn set_bluetooth_enabled(
        service: &BluetoothService,
        enabled: bool,
    ) -> Result<BluetoothSnapshot, String> {
        service.log("info", "radio.toggle.request", format!("enabled={enabled}"));
        let radios = Radio::GetRadiosAsync()
            .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?
            .get()
            .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?;
        let mut found = false;
        for radio in &radios {
            if radio.Kind().ok() != Some(RadioKind::Bluetooth) {
                continue;
            }
            found = true;
            let result = radio
                .SetStateAsync(if enabled {
                    RadioState::On
                } else {
                    RadioState::Off
                })
                .map_err(|_| "BLUETOOTH_RADIO_ACCESS_DENIED".to_string())?
                .get()
                .map_err(|_| "BLUETOOTH_RADIO_ACCESS_DENIED".to_string())?;
            if result.0 != 1 {
                return Err("BLUETOOTH_RADIO_ACCESS_DENIED".to_string());
            }
        }
        if !found {
            return Err("BLUETOOTH_UNAVAILABLE".to_string());
        }
        thread::sleep(Duration::from_millis(180));
        let result = get_bluetooth_state(service);
        service.log(
            if result.is_ok() { "info" } else { "error" },
            "radio.toggle.complete",
            format!("enabled={enabled} success={}", result.is_ok()),
        );
        result
    }

    pub fn start_bluetooth_discovery(
        service: &BluetoothService,
    ) -> Result<BluetoothSnapshot, String> {
        service.log("info", "discovery.start", "watchers=classic,ble");
        stop_bluetooth_discovery(service)?;
        let state = Arc::new(Mutex::new(DiscoveryState {
            active: true,
            devices: BTreeMap::new(),
            watchers: Vec::new(),
        }));
        let selectors = [
            WinBluetoothDevice::GetDeviceSelectorFromPairingState(false)
                .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?,
            BluetoothLEDevice::GetDeviceSelectorFromPairingState(false)
                .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?,
        ];
        let mut watchers = Vec::new();
        for selector in selectors {
            let additional_properties = windows_collections::IIterable::<HSTRING>::from(vec![
                HSTRING::from("System.Devices.Aep.IsPresent"),
                HSTRING::from("System.Devices.AepContainer.IsPresent"),
            ]);
            let watcher = DeviceInformation::CreateWatcherWithKindAqsFilterAndAdditionalProperties(
                &selector,
                &additional_properties,
                DeviceInformationKind::AssociationEndpoint,
            )
            .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?;
            let callback_state = Arc::clone(&state);
            let log_sink = service.log_sink();
            let added_handler = TypedEventHandler::new(
                move |_: Ref<'_, DeviceWatcher>, info: Ref<'_, DeviceInformation>| {
                    let Some(info) = (&*info).as_ref() else {
                        return Ok(());
                    };
                    let name = info
                        .Name()
                        .map(|value| value.to_string())
                        .unwrap_or_else(|_| "Dispositivo Bluetooth".to_string());
                    let presence = device_presence_details(info);
                    let can_pair = info
                        .Pairing()
                        .ok()
                        .and_then(|pairing| pairing.CanPair().ok());
                    match device_from_information(info, false) {
                        Ok(device) => {
                            log_sink.log(
                                "info",
                                "discovery.device_added",
                                format!(
                                    "name={name} can_pair={can_pair:?} presence_aep={:?} presence_container={:?}",
                                    presence.0, presence.1
                                ),
                            );
                            if let Ok(mut state) = callback_state.lock() {
                                state.devices.insert(device.id.clone(), device);
                            }
                        }
                        Err(error) => log_sink.log(
                            "warn",
                            "discovery.device_rejected",
                            format!(
                                "name={name} can_pair={can_pair:?} presence_aep={:?} presence_container={:?} error={error}",
                                presence.0, presence.1
                            ),
                        ),
                    }
                    Ok(())
                },
            );
            watcher
                .Added(&added_handler)
                .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?;
            let removed_handler = TypedEventHandler::new({
                let callback_state = Arc::clone(&state);
                move |_: Ref<'_, DeviceWatcher>, update: Ref<'_, windows::Devices::Enumeration::DeviceInformationUpdate>| {
                    let Some(update) = (&*update).as_ref() else {
                        return Ok(());
                    };
                    if let Ok(id) = update.Id() {
                        if let Ok(mut state) = callback_state.lock() {
                            state.devices.remove(id.to_string().as_str());
                        }
                    }
                    Ok(())
                }
            });
            watcher
                .Removed(&removed_handler)
                .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?;
            watcher
                .Start()
                .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?;
            watchers.push(watcher);
        }
        state
            .lock()
            .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?
            .watchers = watchers;
        *service
            .discovery
            .lock()
            .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())? = Some(state);
        set_pairing_state(service, "discovery", PairingLifecycleState::Discovering);
        let result = get_bluetooth_state(service);
        service.log(
            if result.is_ok() { "info" } else { "error" },
            "discovery.start.complete",
            format!("success={}", result.is_ok()),
        );
        result
    }

    pub fn stop_bluetooth_discovery(
        service: &BluetoothService,
    ) -> Result<BluetoothSnapshot, String> {
        service.log("info", "discovery.stop", "");
        let previous = service
            .discovery
            .lock()
            .map_err(|_| "BLUETOOTH_DISCOVERY_FAILED".to_string())?
            .take();
        if let Some(state) = previous {
            if let Ok(mut state) = state.lock() {
                for watcher in &state.watchers {
                    watcher
                        .Stop()
                        .map_err(|_| "BLUETOOTH_DISCOVERY_STOP_FAILED".to_string())?;
                    wait_for_watcher_stop(service, watcher)?;
                }
                state.active = false;
                state.watchers.clear();
            }
        }
        if service
            .pairing_state
            .lock()
            .map(|state| *state == PairingLifecycleState::Discovering)
            .unwrap_or(false)
        {
            set_pairing_state(service, "discovery", PairingLifecycleState::Idle);
        }
        let result = get_bluetooth_state(service);
        service.log(
            if result.is_ok() { "info" } else { "error" },
            "discovery.stop.complete",
            format!("success={}", result.is_ok()),
        );
        result
    }

    pub fn pair_bluetooth_device(
        service: &BluetoothService,
        device_id: String,
    ) -> Result<BluetoothSnapshot, String> {
        let attempt_id = format!(
            "pair-{}",
            service.pairing_sequence.fetch_add(1, Ordering::Relaxed) + 1
        );
        let attempt_started_at = Instant::now();
        let fingerprint = device_fingerprint(&device_id);
        service.log(
            "info",
            "pair.attempt.start",
            format!(
                "attempt_id={attempt_id} device_fingerprint={fingerprint} device_id_length={}",
                device_id.len()
            ),
        );
        let _pairing_guard = match service.pairing_lock.try_lock() {
            Ok(guard) => {
                service.log(
                    "info",
                    "pair.operation.previous_active",
                    format!("attempt_id={attempt_id} active=false"),
                );
                guard
            }
            Err(TryLockError::WouldBlock) => {
                service.log(
                    "warn",
                    "pair.operation.previous_active",
                    format!("attempt_id={attempt_id} active=true"),
                );
                return Err("BLUETOOTH_PAIRING_IN_PROGRESS".to_string());
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err("BLUETOOTH_PAIRING_FAILED".to_string());
            }
        };
        let mut attempt = PairingAttemptGuard {
            service,
            attempt_id: attempt_id.clone(),
            started_at: attempt_started_at,
            success: false,
        };
        set_pairing_state(service, &attempt_id, PairingLifecycleState::PreparingPair);
        let discovery_was_active = service
            .discovery
            .lock()
            .map_err(|_| "BLUETOOTH_PAIRING_FAILED".to_string())?
            .is_some();
        if discovery_was_active {
            service.log(
                "info",
                "pair.discovery.stop.request",
                format!("attempt_id={attempt_id}"),
            );
            stop_bluetooth_discovery(service)?;
            service.log(
                "info",
                "pair.discovery.stop.complete",
                format!("attempt_id={attempt_id} barrier=watcher_status_terminal"),
            );
        }
        set_pairing_state(service, &attempt_id, PairingLifecycleState::Pairing);
        let first = run_pair_attempt(service, &device_id, &attempt_id)?;
        let first_status = match first {
            PairAttemptResult::AlreadyPaired => {
                let snapshot = reconcile_pairing(service, &device_id, &attempt_id)?;
                attempt.mark_success();
                return Ok(snapshot);
            }
            PairAttemptResult::Status(status) => status,
        };
        if is_success_status(first_status) {
            let snapshot = reconcile_pairing(service, &device_id, &attempt_id)?;
            attempt.mark_success();
            return Ok(snapshot);
        }
        if first_status != DevicePairingResultStatus::OperationAlreadyInProgress {
            return Err(pairing_status_error(first_status));
        }

        service.log(
            "warn",
            "pair.conflict",
            format!(
                "attempt_id={attempt_id} local_operation=false classification=OS_BUSY status={}",
                pairing_status_name(first_status)
            ),
        );
        set_pairing_state(service, &attempt_id, PairingLifecycleState::RetryWait);
        service.log(
            "info",
            "pair.retry.scheduled",
            format!("attempt_id={attempt_id} delay_ms=900 retry_number=2"),
        );
        thread::sleep(Duration::from_millis(900));
        service.log(
            "info",
            "pair.retry.start",
            format!("attempt_id={attempt_id} retry_number=2"),
        );
        set_pairing_state(service, &attempt_id, PairingLifecycleState::Pairing);
        let second = run_pair_attempt(service, &device_id, &attempt_id)?;
        let second_status = match second {
            PairAttemptResult::AlreadyPaired => {
                let snapshot = reconcile_pairing(service, &device_id, &attempt_id)?;
                attempt.mark_success();
                return Ok(snapshot);
            }
            PairAttemptResult::Status(status) => status,
        };
        if is_success_status(second_status) {
            let snapshot = reconcile_pairing(service, &device_id, &attempt_id)?;
            attempt.mark_success();
            return Ok(snapshot);
        }
        if second_status != DevicePairingResultStatus::OperationAlreadyInProgress {
            return Err(pairing_status_error(second_status));
        }

        service.log(
            "error",
            "pair.conflict",
            format!(
                "attempt_id={attempt_id} local_operation=false classification=OS_STUCK status={}",
                pairing_status_name(second_status)
            ),
        );
        set_pairing_state(
            service,
            &attempt_id,
            PairingLifecycleState::RecoveringWindowsAssociation,
        );
        service.log(
            "info",
            "pair.discovery.stop.request",
            format!("attempt_id={attempt_id} reason=das_recovery"),
        );
        stop_bluetooth_discovery(service)?;
        service.log(
            "info",
            "pair.discovery.stop.complete",
            format!("attempt_id={attempt_id} reason=das_recovery"),
        );
        recover_device_association_service(service, &attempt_id)?;
        let (adapters, available, enabled) = read_adapters()?;
        service.log(
            "info",
            "pair.recovery.reenumerate",
            format!(
                "attempt_id={attempt_id} adapters={} available={available} enabled={enabled}",
                adapters.len()
            ),
        );
        let refreshed = start_bluetooth_discovery(service)?;
        service.log(
            "info",
            "pair.recovery.discovery.recreated",
            format!(
                "attempt_id={attempt_id} devices={} discovery_active={}",
                refreshed.devices.len(),
                refreshed.discovery_active
            ),
        );
        stop_bluetooth_discovery(service)?;
        let final_attempt = run_pair_attempt(service, &device_id, &attempt_id)?;
        let final_status = match final_attempt {
            PairAttemptResult::AlreadyPaired => {
                let snapshot = reconcile_pairing(service, &device_id, &attempt_id)?;
                attempt.mark_success();
                return Ok(snapshot);
            }
            PairAttemptResult::Status(status) => status,
        };
        if is_success_status(final_status) {
            let snapshot = reconcile_pairing(service, &device_id, &attempt_id)?;
            attempt.mark_success();
            return Ok(snapshot);
        }
        if final_status == DevicePairingResultStatus::OperationAlreadyInProgress {
            service.log(
                "error",
                "pair.final_conflict",
                format!("attempt_id={attempt_id} recovery_attempted=true"),
            );
        }
        Err(pairing_status_error(final_status))
    }

    fn run_pair_attempt(
        service: &BluetoothService,
        device_id: &str,
        attempt_id: &str,
    ) -> Result<PairAttemptResult, String> {
        let info = recreate_device_information(service, device_id, attempt_id)?;
        let device_name = info
            .Name()
            .map(|name| name.to_string())
            .unwrap_or_else(|_| "Dispositivo Bluetooth".to_string());
        let pairing = info
            .Pairing()
            .map_err(|_| "BLUETOOTH_PAIRING_FAILED".to_string())?;
        let is_paired = pairing.IsPaired().unwrap_or(false);
        let can_pair = pairing.CanPair().unwrap_or(false);
        let is_enabled = info.IsEnabled().unwrap_or(false);
        let is_present = device_presence(&info);
        service.log(
            "info",
            "pair.preflight",
            format!(
                "attempt_id={attempt_id} device_fingerprint={} name={device_name} paired={is_paired} can_pair={can_pair} enabled={is_enabled} present={is_present:?}",
                device_fingerprint(device_id)
            ),
        );
        if is_paired {
            return Ok(PairAttemptResult::AlreadyPaired);
        }
        if !can_pair || is_present == Some(false) {
            return Err("BLUETOOTH_DEVICE_NOT_READY".to_string());
        }
        let status = pair_once_custom(service, &info, attempt_id)?;
        Ok(PairAttemptResult::Status(status))
    }

    fn recreate_device_information(
        service: &BluetoothService,
        device_id: &str,
        attempt_id: &str,
    ) -> Result<DeviceInformation, String> {
        let additional_properties = windows_collections::IIterable::<HSTRING>::from(vec![
            HSTRING::from("System.Devices.Aep.IsPresent"),
            HSTRING::from("System.Devices.AepContainer.IsPresent"),
        ]);
        let info = DeviceInformation::CreateFromIdAsyncWithKindAndAdditionalProperties(
            &HSTRING::from(device_id),
            &additional_properties,
            DeviceInformationKind::AssociationEndpoint,
        )
        .map_err(|_| "BLUETOOTH_DEVICE_GONE".to_string())?
        .get()
        .map_err(|_| "BLUETOOTH_DEVICE_GONE".to_string())?;
        service.log(
            "info",
            "pair.device.recreated",
            format!(
                "attempt_id={attempt_id} kind=AssociationEndpoint device_fingerprint={} present={:?}",
                device_fingerprint(device_id),
                device_presence(&info)
            ),
        );
        Ok(info)
    }

    fn pair_once_custom(
        service: &BluetoothService,
        info: &DeviceInformation,
        attempt_id: &str,
    ) -> Result<DevicePairingResultStatus, String> {
        let pairing = info
            .Pairing()
            .map_err(|_| "BLUETOOTH_PAIRING_FAILED".to_string())?;
        let custom = pairing
            .Custom()
            .map_err(|_| "BLUETOOTH_PAIRING_FAILED".to_string())?;
        let unsupported_ceremony = Arc::new(AtomicBool::new(false));
        let ceremony_log = service.log_sink();
        let ceremony_attempt_id = attempt_id.to_string();
        let ceremony_unsupported = Arc::clone(&unsupported_ceremony);
        let handler = TypedEventHandler::new(
            move |_: Ref<'_, DeviceInformationCustomPairing>,
                  args: Ref<'_, DevicePairingRequestedEventArgs>| {
                let Some(args) = (&*args).as_ref() else {
                    return Ok(());
                };
                let kind = match args.PairingKind() {
                    Ok(kind) => kind,
                    Err(error) => {
                        ceremony_log.log(
                            "error",
                            "pair.ceremony.error",
                            format!("attempt_id={ceremony_attempt_id} error={error:?}"),
                        );
                        return Ok(());
                    }
                };
                let kind_name = pairing_kind_name(kind);
                ceremony_log.log(
                    "info",
                    "pair.ceremony.requested",
                    format!("attempt_id={ceremony_attempt_id} kind={kind_name}"),
                );
                if kind == DevicePairingKinds::ConfirmOnly {
                    args.Accept().map_err(|error| {
                        ceremony_log.log(
                            "error",
                            "pair.ceremony.accept_failed",
                            format!("attempt_id={ceremony_attempt_id} error={error:?}"),
                        );
                        error
                    })?;
                    return Ok(());
                }
                ceremony_unsupported.store(true, Ordering::Release);
                ceremony_log.log(
                    "warn",
                    "pair.ceremony.interaction_required",
                    format!("attempt_id={ceremony_attempt_id} kind={kind_name}"),
                );
                Ok(())
            },
        );
        let handler_registered_at = Instant::now();
        let token = custom
            .PairingRequested(&handler)
            .map_err(|_| "BLUETOOTH_PAIRING_FAILED".to_string())?;
        service.log(
            "info",
            "pair.handler.registered",
            format!("attempt_id={attempt_id}"),
        );
        set_pairing_state(
            service,
            attempt_id,
            PairingLifecycleState::WaitingForInteraction,
        );
        let supported_kinds = DevicePairingKinds(0x0F);
        let operation_created_at = Instant::now();
        service.log(
            "info",
            "pair.async.created",
            format!("attempt_id={attempt_id} supported_kinds=ConfirmOnly|DisplayPin|ProvidePin|ConfirmPinMatch"),
        );
        let operation = match custom.PairAsync(supported_kinds) {
            Ok(operation) => operation,
            Err(error) => {
                let _ = custom.RemovePairingRequested(token);
                return Err(format!("BLUETOOTH_PAIRING_FAILED:{error:?}"));
            }
        };
        service.log(
            "info",
            "pair.async.started",
            format!("attempt_id={attempt_id}"),
        );
        let result = wait_for_pairing_operation(service, operation, attempt_id);
        let _ = custom.RemovePairingRequested(token);
        let elapsed_pairasync_to_result_ms = operation_created_at.elapsed().as_millis();
        match result {
            Ok(result) => {
                let status = result
                    .Status()
                    .map_err(|error| format!("BLUETOOTH_PAIRING_FAILED:{error:?}"))?;
                service.log(
                    if is_success_status(status) { "info" } else { "error" },
                    "pair.result",
                    format!(
                        "attempt_id={attempt_id} status={} pairasync_to_result_ms={elapsed_pairasync_to_result_ms} handler_to_result_ms={}",
                        pairing_status_name(status),
                        handler_registered_at.elapsed().as_millis()
                    ),
                );
                if unsupported_ceremony.load(Ordering::Acquire) {
                    return Err("BLUETOOTH_PAIRING_INTERACTION_REQUIRED".to_string());
                }
                Ok(status)
            }
            Err(error) => {
                service.log(
                    "error",
                    "pair.result",
                    format!(
                        "attempt_id={attempt_id} status_error={error} pairasync_to_result_ms={elapsed_pairasync_to_result_ms} handler_to_result_ms={}",
                        handler_registered_at.elapsed().as_millis()
                    ),
                );
                Err(error)
            }
        }
    }

    fn device_fingerprint(device_id: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        device_id.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn is_success_status(status: DevicePairingResultStatus) -> bool {
        status == DevicePairingResultStatus::Paired
            || status == DevicePairingResultStatus::AlreadyPaired
    }

    fn pairing_status_error(status: DevicePairingResultStatus) -> String {
        match status {
            DevicePairingResultStatus::ConnectionRejected
            | DevicePairingResultStatus::PairingCanceled
            | DevicePairingResultStatus::RejectedByHandler => {
                "BLUETOOTH_PAIRING_REJECTED".to_string()
            }
            DevicePairingResultStatus::NotReadyToPair | DevicePairingResultStatus::Failed => {
                "BLUETOOTH_DEVICE_NOT_READY".to_string()
            }
            DevicePairingResultStatus::OperationAlreadyInProgress => {
                "BLUETOOTH_PAIRING_FAILED".to_string()
            }
            _ => "BLUETOOTH_PAIRING_FAILED".to_string(),
        }
    }

    fn reconcile_pairing(
        service: &BluetoothService,
        device_id: &str,
        attempt_id: &str,
    ) -> Result<BluetoothSnapshot, String> {
        thread::sleep(Duration::from_millis(180));
        let snapshot = get_bluetooth_state(service)?;
        let paired = snapshot
            .devices
            .iter()
            .filter(|device| device.paired)
            .count();
        let connected = snapshot
            .devices
            .iter()
            .filter(|device| device.connected)
            .count();
        let target_seen = snapshot.devices.iter().any(|device| device.id == device_id);
        service.log(
            "info",
            "pair.reconcile",
            format!(
                "attempt_id={attempt_id} device_fingerprint={} target_seen={target_seen} paired_count={paired} connected_count={connected}",
                device_fingerprint(device_id)
            ),
        );
        Ok(snapshot)
    }

    fn wait_for_pairing_operation(
        service: &BluetoothService,
        operation: windows_future::IAsyncOperation<
            windows::Devices::Enumeration::DevicePairingResult,
        >,
        attempt_id: &str,
    ) -> Result<windows::Devices::Enumeration::DevicePairingResult, String> {
        const OPERATION_TIMEOUT: Duration = Duration::from_secs(45);
        const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(10);
        let started_at = Instant::now();
        let mut cancel_requested_at = None;
        service.log(
            "info",
            "pair.operation.waiting",
            format!("attempt_id={attempt_id}"),
        );

        loop {
            let status = operation
                .Status()
                .map_err(|error| format!("BLUETOOTH_PAIRING_FAILED:{error:?}"))?;
            if status == windows_future::AsyncStatus::Started {
                if cancel_requested_at.is_none() && started_at.elapsed() >= OPERATION_TIMEOUT {
                    set_pairing_state(service, attempt_id, PairingLifecycleState::Cancelling);
                    service.log(
                        "warn",
                        "pair.operation.timeout",
                        format!("attempt_id={attempt_id} timeout_ms=45000"),
                    );
                    service.log(
                        "info",
                        "pair.operation.cancel.requested",
                        format!("attempt_id={attempt_id}"),
                    );
                    let cancel_result = operation.Cancel();
                    if let Err(error) = cancel_result {
                        service.log(
                            "error",
                            "pair.operation.cancel.complete",
                            format!("attempt_id={attempt_id} success=false error={error:?}"),
                        );
                        let _ = operation.Close();
                        service.log(
                            "error",
                            "pair.operation.closed",
                            format!("attempt_id={attempt_id} success=true after_cancel_error=true"),
                        );
                        return Err("BLUETOOTH_PAIRING_CLEANUP_FAILED".to_string());
                    }
                    cancel_requested_at = Some(Instant::now());
                }
                if let Some(cancel_started_at) = cancel_requested_at {
                    if cancel_started_at.elapsed() >= CANCELLATION_TIMEOUT {
                        service.log(
                            "error",
                            "pair.operation.cancel.complete",
                            format!(
                                "attempt_id={attempt_id} success=false reason=terminal_state_timeout"
                            ),
                        );
                        let _ = operation.Close();
                        service.log(
                            "error",
                            "pair.operation.closed",
                            format!("attempt_id={attempt_id} success=true forced_after_cancel_timeout=true"),
                        );
                        return Err("BLUETOOTH_PAIRING_CLEANUP_FAILED".to_string());
                    }
                }
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let status_name = match status {
                windows_future::AsyncStatus::Completed => "Completed",
                windows_future::AsyncStatus::Canceled => "Canceled",
                windows_future::AsyncStatus::Error => "Error",
                _ => "Unknown",
            };
            service.log(
                if status == windows_future::AsyncStatus::Completed {
                    "info"
                } else {
                    "error"
                },
                "pair.operation.completed",
                format!("attempt_id={attempt_id} status={status_name}"),
            );
            if cancel_requested_at.is_some() {
                service.log(
                    "info",
                    "pair.operation.cancel.complete",
                    format!("attempt_id={attempt_id} success=true status={status_name}"),
                );
            }

            let result = if status == windows_future::AsyncStatus::Completed {
                operation
                    .GetResults()
                    .map_err(|error| format!("BLUETOOTH_PAIRING_FAILED:{error:?}"))
            } else if status == windows_future::AsyncStatus::Canceled {
                Err("BLUETOOTH_PAIRING_TIMEOUT".to_string())
            } else {
                let error_code = operation
                    .ErrorCode()
                    .map(|error| format!("{error:?}"))
                    .unwrap_or_else(|_| "unknown".to_string());
                Err(format!("BLUETOOTH_PAIRING_FAILED:{error_code}"))
            };
            let close_result = operation.Close();
            service.log(
                if close_result.is_ok() {
                    "info"
                } else {
                    "error"
                },
                "pair.operation.closed",
                format!("attempt_id={attempt_id} success={}", close_result.is_ok()),
            );
            return result;
        }
    }

    fn wait_for_watcher_stop(
        service: &BluetoothService,
        watcher: &DeviceWatcher,
    ) -> Result<(), String> {
        const STOP_TIMEOUT: Duration = Duration::from_secs(5);
        let started_at = Instant::now();
        loop {
            let status = watcher
                .Status()
                .map_err(|_| "BLUETOOTH_DISCOVERY_STOP_FAILED".to_string())?;
            if status == DeviceWatcherStatus::Stopped || status == DeviceWatcherStatus::Aborted {
                service.log(
                    "info",
                    "discovery.watcher_stopped",
                    format!("status={status:?}"),
                );
                return Ok(());
            }
            if started_at.elapsed() >= STOP_TIMEOUT {
                service.log(
                    "error",
                    "discovery.watcher_stop_timeout",
                    format!("status={status:?} timeout_ms=5000"),
                );
                return Err("BLUETOOTH_DISCOVERY_STOP_TIMEOUT".to_string());
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn pairing_kind_name(kind: DevicePairingKinds) -> String {
        let mut names = Vec::new();
        if kind.contains(DevicePairingKinds::ConfirmOnly) {
            names.push("ConfirmOnly");
        }
        if kind.contains(DevicePairingKinds::DisplayPin) {
            names.push("DisplayPin");
        }
        if kind.contains(DevicePairingKinds::ConfirmPinMatch) {
            names.push("ConfirmPinMatch");
        }
        if kind.contains(DevicePairingKinds::ProvidePin) {
            names.push("ProvidePin");
        }
        if names.is_empty() {
            "None".to_string()
        } else {
            names.join("|")
        }
    }

    struct ServiceHandle(SC_HANDLE);

    impl Drop for ServiceHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = CloseServiceHandle(self.0);
                }
            }
        }
    }

    pub fn maybe_run_elevated_bluetooth_recovery_helper() -> bool {
        if !std::env::args().any(|arg| arg == "--lumadeck-recover-das") {
            return false;
        }
        let service = BluetoothService::default();
        let result = recover_device_association_service_with_policy(&service, "elevated", false);
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }

    fn recover_device_association_service(
        service: &BluetoothService,
        attempt_id: &str,
    ) -> Result<(), String> {
        recover_device_association_service_with_policy(service, attempt_id, true)
    }

    fn recover_device_association_service_with_policy(
        service: &BluetoothService,
        attempt_id: &str,
        allow_elevation: bool,
    ) -> Result<(), String> {
        service.log(
            "info",
            "pair.recovery.das.start",
            format!("attempt_id={attempt_id} service=DeviceAssociationService"),
        );
        let manager = ServiceHandle(unsafe {
            OpenSCManagerW(
                std::ptr::null(),
                SERVICES_ACTIVE_DATABASE,
                SC_MANAGER_CONNECT,
            )
        });
        if manager.0.is_null() {
            let error_code = unsafe { GetLastError() };
            return if allow_elevation && error_code == ERROR_ACCESS_DENIED {
                run_elevated_recovery_helper(service, attempt_id)
            } else {
                recovery_service_error(service, attempt_id, "open_scm", error_code)
            };
        }
        let service_handle = ServiceHandle(unsafe {
            OpenServiceW(
                manager.0,
                windows_sys::core::w!("DeviceAssociationService"),
                SERVICE_QUERY_STATUS | SERVICE_STOP | SERVICE_START,
            )
        });
        if service_handle.0.is_null() {
            let error_code = unsafe { GetLastError() };
            return if allow_elevation && error_code == ERROR_ACCESS_DENIED {
                run_elevated_recovery_helper(service, attempt_id)
            } else {
                recovery_service_error(service, attempt_id, "open_service", error_code)
            };
        }

        let current = query_service_state(service_handle.0).map_err(|code| {
            service.log(
                "error",
                "pair.recovery.das.error",
                format!("attempt_id={attempt_id} phase=query_initial error_code={code}"),
            );
            "BLUETOOTH_PAIRING_RECOVERY_FAILED".to_string()
        })?;
        service.log(
            "info",
            "pair.recovery.das.status",
            format!("attempt_id={attempt_id} phase=initial state={current}"),
        );
        if current != SERVICE_STOPPED {
            service.log(
                "info",
                "pair.recovery.das.stop_requested",
                format!("attempt_id={attempt_id} state={current}"),
            );
            let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
            let stopped =
                unsafe { ControlService(service_handle.0, SERVICE_CONTROL_STOP, &mut status) };
            if stopped == 0 {
                let error_code = unsafe { GetLastError() };
                if error_code != ERROR_SERVICE_NOT_ACTIVE {
                    return recovery_service_error(service, attempt_id, "stop", error_code);
                }
            }
        } else {
            service.log(
                "info",
                "pair.recovery.das.stop_requested",
                format!("attempt_id={attempt_id} already_stopped=true"),
            );
        }
        wait_for_service_state(
            service,
            attempt_id,
            service_handle.0,
            SERVICE_STOPPED,
            "stopped",
        )?;

        service.log(
            "info",
            "pair.recovery.das.start_requested",
            format!("attempt_id={attempt_id}"),
        );
        let started = unsafe { StartServiceW(service_handle.0, 0, std::ptr::null()) };
        if started == 0 {
            let error_code = unsafe { GetLastError() };
            if error_code != ERROR_SERVICE_ALREADY_RUNNING {
                return recovery_service_error(service, attempt_id, "start", error_code);
            }
        }
        wait_for_service_state(
            service,
            attempt_id,
            service_handle.0,
            SERVICE_RUNNING,
            "running",
        )?;
        service.log(
            "info",
            "pair.recovery.das.complete",
            format!("attempt_id={attempt_id} success=true"),
        );
        Ok(())
    }

    fn run_elevated_recovery_helper(
        service: &BluetoothService,
        attempt_id: &str,
    ) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;

        service.log(
            "warn",
            "pair.recovery.elevated.start",
            format!("attempt_id={attempt_id} reason=access_denied"),
        );
        let executable = std::env::current_exe().map_err(|error| {
            service.log(
                "error",
                "pair.recovery.elevated.error",
                format!("attempt_id={attempt_id} phase=current_exe error={error}"),
            );
            "BLUETOOTH_PAIRING_RECOVERY_FAILED".to_string()
        })?;
        let executable: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let parameters: Vec<u16> = "--lumadeck-recover-das"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
        let mut execute_info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        execute_info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        execute_info.fMask = SEE_MASK_NOCLOSEPROCESS;
        execute_info.lpVerb = verb.as_ptr();
        execute_info.lpFile = executable.as_ptr();
        execute_info.lpParameters = parameters.as_ptr();
        execute_info.nShow = 0;
        let launched = unsafe { ShellExecuteExW(&mut execute_info) };
        if launched == 0 || execute_info.hProcess.is_null() {
            let error_code = unsafe { GetLastError() };
            service.log(
                "error",
                "pair.recovery.elevated.error",
                format!("attempt_id={attempt_id} phase=launch error_code={error_code}"),
            );
            return Err("BLUETOOTH_PAIRING_RECOVERY_FAILED".to_string());
        }
        unsafe {
            let _ = WaitForSingleObject(execute_info.hProcess, INFINITE);
        }
        let mut exit_code = 1u32;
        let read_exit_code = unsafe { GetExitCodeProcess(execute_info.hProcess, &mut exit_code) };
        unsafe {
            let _ = CloseHandle(execute_info.hProcess);
        }
        if read_exit_code == 0 || exit_code != 0 {
            service.log(
                "error",
                "pair.recovery.elevated.error",
                format!("attempt_id={attempt_id} phase=complete exit_code={exit_code}"),
            );
            return Err("BLUETOOTH_PAIRING_RECOVERY_FAILED".to_string());
        }
        service.log(
            "info",
            "pair.recovery.elevated.complete",
            format!("attempt_id={attempt_id} success=true"),
        );
        Ok(())
    }

    fn query_service_state(handle: SC_HANDLE) -> Result<u32, u32> {
        let mut status: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
        let mut bytes_needed = 0;
        let result = unsafe {
            QueryServiceStatusEx(
                handle,
                SC_STATUS_PROCESS_INFO,
                (&mut status as *mut SERVICE_STATUS_PROCESS).cast::<u8>(),
                std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
                &mut bytes_needed,
            )
        };
        if result == 0 {
            Err(unsafe { GetLastError() })
        } else {
            Ok(status.dwCurrentState)
        }
    }

    fn wait_for_service_state(
        service: &BluetoothService,
        attempt_id: &str,
        handle: SC_HANDLE,
        expected: u32,
        phase: &str,
    ) -> Result<(), String> {
        const SERVICE_TIMEOUT: Duration = Duration::from_secs(15);
        let started_at = Instant::now();
        loop {
            let state = query_service_state(handle).map_err(|code| {
                service.log(
                    "error",
                    "pair.recovery.das.error",
                    format!("attempt_id={attempt_id} phase=query_{phase} error_code={code}"),
                );
                "BLUETOOTH_PAIRING_RECOVERY_FAILED".to_string()
            })?;
            service.log(
                "info",
                "pair.recovery.das.status",
                format!("attempt_id={attempt_id} phase={phase} state={state}"),
            );
            if state == expected {
                service.log(
                    "info",
                    format!("pair.recovery.das.{phase}").as_str(),
                    format!("attempt_id={attempt_id} state={state}"),
                );
                return Ok(());
            }
            if started_at.elapsed() >= SERVICE_TIMEOUT {
                return recovery_service_error(service, attempt_id, phase, state);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn recovery_service_error(
        service: &BluetoothService,
        attempt_id: &str,
        phase: &str,
        error_code: u32,
    ) -> Result<(), String> {
        service.log(
            "error",
            "pair.recovery.das.error",
            format!("attempt_id={attempt_id} phase={phase} error_code={error_code}"),
        );
        Err("BLUETOOTH_PAIRING_RECOVERY_FAILED".to_string())
    }

    pub fn unpair_bluetooth_device(
        service: &BluetoothService,
        device_id: String,
    ) -> Result<BluetoothSnapshot, String> {
        service.log(
            "info",
            "unpair.start",
            format!("device_id_length={}", device_id.len()),
        );
        let info = DeviceInformation::CreateFromIdAsync(&HSTRING::from(device_id))
            .map_err(|_| "BLUETOOTH_DEVICE_GONE".to_string())?
            .get()
            .map_err(|_| "BLUETOOTH_DEVICE_GONE".to_string())?;
        let result = info
            .Pairing()
            .map_err(|_| "BLUETOOTH_FORGET_FAILED".to_string())?
            .UnpairAsync()
            .map_err(|_| "BLUETOOTH_FORGET_FAILED".to_string())?
            .get()
            .map_err(|_| "BLUETOOTH_FORGET_FAILED".to_string())?;
        let unpairing_status = result.Status();
        service.log(
            if unpairing_status.is_ok() {
                "info"
            } else {
                "error"
            },
            "unpair.result",
            format!("status={unpairing_status:?}"),
        );
        match unpairing_status {
            Ok(status)
                if status == DeviceUnpairingResultStatus::Unpaired
                    || status == DeviceUnpairingResultStatus::AlreadyUnpaired =>
            {
                thread::sleep(Duration::from_millis(180));
                get_bluetooth_state(service)
            }
            _ => Err("BLUETOOTH_FORGET_FAILED".to_string()),
        }
    }

    fn read_adapters() -> Result<(Vec<BluetoothAdapter>, bool, bool), String> {
        let radios = Radio::GetRadiosAsync()
            .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?
            .get()
            .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?;
        let mut adapters = Vec::new();
        for (index, radio) in (&radios).into_iter().enumerate() {
            if radio.Kind().ok() != Some(RadioKind::Bluetooth) {
                continue;
            }
            let state = radio.State().unwrap_or(RadioState::Unknown);
            let enabled = state == RadioState::On;
            let name = radio
                .Name()
                .map(|value| value.to_string())
                .unwrap_or_else(|_| "Adaptador Bluetooth".to_string());
            adapters.push(BluetoothAdapter {
                id: format!("bluetooth-radio-{index}"),
                name,
                enabled,
                available: state != RadioState::Disabled,
                discoverable: false,
                hardware_present: true,
                status: if enabled { "enabled" } else { "disabled" }.to_string(),
                error: None,
            });
        }
        let available = !adapters.is_empty();
        let enabled = adapters.iter().any(|adapter| adapter.enabled);
        Ok((adapters, available, enabled))
    }

    fn pairing_status_name(status: DevicePairingResultStatus) -> &'static str {
        match status {
            DevicePairingResultStatus::Paired => "Paired",
            DevicePairingResultStatus::NotReadyToPair => "NotReadyToPair",
            DevicePairingResultStatus::NotPaired => "NotPaired",
            DevicePairingResultStatus::AlreadyPaired => "AlreadyPaired",
            DevicePairingResultStatus::ConnectionRejected => "ConnectionRejected",
            DevicePairingResultStatus::TooManyConnections => "TooManyConnections",
            DevicePairingResultStatus::HardwareFailure => "HardwareFailure",
            DevicePairingResultStatus::AuthenticationTimeout => "AuthenticationTimeout",
            DevicePairingResultStatus::AuthenticationNotAllowed => "AuthenticationNotAllowed",
            DevicePairingResultStatus::AuthenticationFailure => "AuthenticationFailure",
            DevicePairingResultStatus::NoSupportedProfiles => "NoSupportedProfiles",
            DevicePairingResultStatus::ProtectionLevelCouldNotBeMet => {
                "ProtectionLevelCouldNotBeMet"
            }
            DevicePairingResultStatus::AccessDenied => "AccessDenied",
            DevicePairingResultStatus::InvalidCeremonyData => "InvalidCeremonyData",
            DevicePairingResultStatus::PairingCanceled => "PairingCanceled",
            DevicePairingResultStatus::OperationAlreadyInProgress => "OperationAlreadyInProgress",
            DevicePairingResultStatus::RequiredHandlerNotRegistered => {
                "RequiredHandlerNotRegistered"
            }
            DevicePairingResultStatus::RejectedByHandler => "RejectedByHandler",
            DevicePairingResultStatus::RemoteDeviceHasAssociation => "RemoteDeviceHasAssociation",
            DevicePairingResultStatus::Failed => "Failed",
            _ => "Unknown",
        }
    }

    fn read_paired_devices() -> Result<Vec<BluetoothDevice>, String> {
        let selectors = [
            WinBluetoothDevice::GetDeviceSelectorFromPairingState(true)
                .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?,
            BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)
                .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?,
        ];
        let mut devices = Vec::new();
        for selector in selectors {
            let collection = DeviceInformation::FindAllAsyncAqsFilter(&selector)
                .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?
                .get()
                .map_err(|_| "BLUETOOTH_UNAVAILABLE".to_string())?;
            for info in &collection {
                if let Ok(device) = device_from_information(&info, true) {
                    merge_devices(&mut devices, [device]);
                }
            }
        }
        Ok(devices)
    }

    fn device_from_information(
        info: &DeviceInformation,
        paired_hint: bool,
    ) -> Result<BluetoothDevice, String> {
        let id = info
            .Id()
            .map_err(|_| "BLUETOOTH_DEVICE_GONE".to_string())?
            .to_string();
        let name = info
            .Name()
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "Dispositivo Bluetooth".to_string());
        let pairing = info
            .Pairing()
            .map_err(|_| "BLUETOOTH_DEVICE_GONE".to_string())?;
        let paired = pairing.IsPaired().unwrap_or(paired_hint) || paired_hint;
        let can_pair = pairing.CanPair().unwrap_or(false);
        if !paired_hint && (!can_pair || device_presence(info) != Some(true)) {
            return Err("BLUETOOTH_DEVICE_NOT_READY".to_string());
        }
        let connected = connection_status(&id);
        let class = bluetooth_class(&id);
        Ok(BluetoothDevice {
            id,
            name,
            paired,
            connected,
            connectable: !paired && can_pair,
            signal_strength: None,
            device_class: class,
            battery_level: None,
            last_seen: None,
            pairing_state: if paired { "paired" } else { "unpaired" }.to_string(),
            connection_state: if connected {
                "connected".to_string()
            } else {
                "disconnected".to_string()
            },
        })
    }

    fn device_presence(info: &DeviceInformation) -> Option<bool> {
        let (aep, container) = device_presence_details(info);
        aep.or(container)
    }

    fn device_presence_details(info: &DeviceInformation) -> (Option<bool>, Option<bool>) {
        let Some(properties) = info.Properties().ok() else {
            return (None, None);
        };
        let read_boolean = |key: &str| {
            properties
                .Lookup(&HSTRING::from(key))
                .ok()
                .and_then(|value| value.cast::<IPropertyValue>().ok())
                .and_then(|value| value.GetBoolean().ok())
        };
        (
            read_boolean("System.Devices.Aep.IsPresent"),
            read_boolean("System.Devices.AepContainer.IsPresent"),
        )
    }

    fn connection_status(id: &str) -> bool {
        let id = HSTRING::from(id);
        if let Ok(operation) = WinBluetoothDevice::FromIdAsync(&id) {
            if let Ok(device) = operation.get() {
                if device.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected) {
                    return true;
                }
            }
        }
        if let Ok(operation) = BluetoothLEDevice::FromIdAsync(&id) {
            if let Ok(device) = operation.get() {
                return device.ConnectionStatus().ok()
                    == Some(BluetoothConnectionStatus::Connected);
            }
        }
        false
    }

    fn bluetooth_class(id: &str) -> String {
        let id = HSTRING::from(id);
        let class = WinBluetoothDevice::FromIdAsync(&id)
            .ok()
            .and_then(|operation| operation.get().ok())
            .and_then(|device| device.ClassOfDevice().ok());
        let Some(class) = class else {
            return "other".to_string();
        };
        let major = class
            .MajorClass()
            .unwrap_or(BluetoothMajorClass::Miscellaneous);
        let minor = class
            .MinorClass()
            .unwrap_or(BluetoothMinorClass::Uncategorized);
        if major == BluetoothMajorClass::Peripheral {
            return match minor {
                value
                    if value == BluetoothMinorClass::PeripheralGamepad
                        || value == BluetoothMinorClass::PeripheralJoystick =>
                {
                    "gamepad"
                }
                value if value.0 == 0x40 => "keyboard",
                value if value.0 == 0x80 => "mouse",
                _ => "other",
            }
            .to_string();
        }
        if major == BluetoothMajorClass::AudioVideo {
            return match minor {
                value if value == BluetoothMinorClass::AudioVideoHeadphones => "headphones",
                value
                    if value == BluetoothMinorClass::AudioVideoWearableHeadset
                        || value == BluetoothMinorClass::AudioVideoHandsFree =>
                {
                    "headset"
                }
                value if value == BluetoothMinorClass::AudioVideoLoudspeaker => "speaker",
                _ => "other",
            }
            .to_string();
        }
        if major == BluetoothMajorClass::Phone {
            return "phone".to_string();
        }
        if major == BluetoothMajorClass::Computer {
            return "computer".to_string();
        }
        "other".to_string()
    }

    fn merge_devices<I>(devices: &mut Vec<BluetoothDevice>, incoming: I)
    where
        I: IntoIterator<Item = BluetoothDevice>,
    {
        for incoming in incoming {
            if let Some(current) = devices.iter_mut().find(|device| device.id == incoming.id) {
                current.paired |= incoming.paired;
                current.connected |= incoming.connected;
                current.connectable |= incoming.connectable;
                if current.device_class == "other" {
                    current.device_class = incoming.device_class;
                }
                current.pairing_state = if current.paired {
                    "paired".to_string()
                } else {
                    "unpaired".to_string()
                };
                current.connection_state = if current.connected {
                    "connected".to_string()
                } else {
                    "disconnected".to_string()
                };
            } else {
                devices.push(incoming);
            }
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{
    get_bluetooth_state, maybe_run_elevated_bluetooth_recovery_helper, pair_bluetooth_device,
    set_bluetooth_enabled, start_bluetooth_discovery, stop_bluetooth_discovery,
    unpair_bluetooth_device,
};
