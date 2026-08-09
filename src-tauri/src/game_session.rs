use crate::frame_generation::FrameGenerationProvider;
use crate::gamepad::{GamepadShortcutMonitor, ShortcutEvent};
use crate::lossless_scaling::{is_lossless_scaling_running, LosslessScalingProvider};
use crate::{eden, launch_display, settings, steam};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;

pub const GAME_SESSION_EVENT: &str = "game-session-state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GameSessionState {
    Idle,
    Preparing,
    Launching,
    Running,
    Finishing,
    Error,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MonitoringMode {
    Full,
    Compatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCapabilities {
    pub playtime: bool,
    pub start_time: bool,
    pub end_time: bool,
    pub process_tracking: bool,
    pub advanced_process_metrics: bool,
}

impl SessionCapabilities {
    fn for_mode(mode: MonitoringMode) -> Self {
        Self {
            playtime: true,
            start_time: true,
            end_time: true,
            process_tracking: true,
            advanced_process_metrics: matches!(mode, MonitoringMode::Full),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSessionStatus {
    pub session_id: String,
    pub game_id: String,
    pub steam_app_id: i64,
    pub source: String,
    pub state: GameSessionState,
    pub occurred_at: String,
    pub elapsed_seconds: i64,
    pub message: String,
    pub unsupported_reason: Option<String>,
    pub monitoring_mode: MonitoringMode,
    pub anti_cheat_provider: Option<String>,
    pub compatible_reason: Option<String>,
    pub capabilities: SessionCapabilities,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SessionCommandError {
    #[error("another game session is active")]
    AnotherGameSessionIsActive,
    #[error("game session is not waiting for dismissal")]
    NotDismissable,
}

#[derive(Clone, Default)]
pub struct SteamGameSessionService {
    active: Arc<Mutex<Option<ActiveSession>>>,
}

#[derive(Debug, Clone)]
struct ActiveSession {
    session_id: String,
    game_id: String,
    steam_app_id: i64,
    source: String,
    state: GameSessionState,
    install_dir: Option<PathBuf>,
    started_at: Option<String>,
    activity_session_id: Option<i64>,
    tracked_processes: HashSet<u32>,
    message: String,
    unsupported_reason: Option<String>,
    monitoring_mode: MonitoringMode,
    anti_cheat_provider: Option<String>,
    compatible_reason: Option<String>,
    capabilities: SessionCapabilities,
    activity_ended: bool,
}

impl SteamGameSessionService {
    pub fn start(
        &self,
        app: AppHandle,
        game_id: String,
    ) -> Result<GameSessionStatus, SessionCommandError> {
        let session_id = format!("play-{}-{}", sanitize_id(&game_id), unix_seconds());
        let active = ActiveSession {
            session_id: session_id.clone(),
            game_id: game_id.clone(),
            steam_app_id: 0,
            source: "unknown".to_string(),
            state: GameSessionState::Preparing,
            install_dir: None,
            started_at: None,
            activity_session_id: None,
            tracked_processes: HashSet::new(),
            message: "Comprobando instalación y compatibilidad…".to_string(),
            unsupported_reason: None,
            monitoring_mode: MonitoringMode::Full,
            anti_cheat_provider: None,
            compatible_reason: None,
            capabilities: SessionCapabilities::for_mode(MonitoringMode::Full),
            activity_ended: false,
        };
        {
            let mut current = self
                .active
                .lock()
                .map_err(|_| SessionCommandError::AnotherGameSessionIsActive)?;
            if current
                .as_ref()
                .is_some_and(|session| is_blocking_state(session.state))
            {
                return Err(SessionCommandError::AnotherGameSessionIsActive);
            }
            *current = Some(active.clone());
        }

        let status = status_from_session(&active);
        emit_status(&app, &status);
        let database = app.state::<settings::DatabaseState>();
        session_log(
            &database,
            &session_id,
            "SESSION_CREATED",
            &format!("gameId={game_id} source=unknown emulator=unknown"),
        );
        let service = self.clone();
        thread::spawn(move || service.run(app, session_id, game_id));
        Ok(status)
    }

    pub fn current_status(&self) -> GameSessionStatus {
        self.active
            .lock()
            .ok()
            .and_then(|current| current.as_ref().map(status_from_session))
            .unwrap_or_else(idle_status)
    }

    pub fn dismiss(&self, app: AppHandle) -> Result<GameSessionStatus, SessionCommandError> {
        let status = {
            let mut current = self
                .active
                .lock()
                .map_err(|_| SessionCommandError::NotDismissable)?;
            let Some(session) = current.as_ref() else {
                return Ok(idle_status());
            };
            if !matches!(
                session.state,
                GameSessionState::Error | GameSessionState::Unsupported
            ) {
                return Err(SessionCommandError::NotDismissable);
            }
            *current = None;
            idle_status()
        };
        emit_status(&app, &status);
        Ok(status)
    }

    fn run(&self, app: AppHandle, session_id: String, game_id: String) {
        let database = app.state::<settings::DatabaseState>();
        let launch_game = match settings::get_launch_game(&database, &game_id) {
            Ok(Some(game)) => game,
            Ok(None) => {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Unsupported,
                    "El juego no está disponible en la biblioteca local.".to_string(),
                    Some("game-not-found".to_string()),
                );
                return;
            }
            Err(error) => {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "No se pudo consultar el destino del juego.".to_string(),
                    Some(format!("database: {error}")),
                );
                return;
            }
        };
        let status_source = if launch_game.source.eq_ignore_ascii_case("emulator") {
            launch_game
                .emulator_id
                .clone()
                .unwrap_or_else(|| launch_game.source.clone())
        } else {
            launch_game.provider.clone()
        };
        self.set_source(&session_id, status_source);
        let emulator_installation_id = if launch_game
            .emulator_id
            .as_deref()
            .is_some_and(|id| id.eq_ignore_ascii_case("eden"))
        {
            eden::installation_id(&database)
                .ok()
                .flatten()
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "unknown".to_string()
        };
        session_log(
            &database,
            &session_id,
            "SESSION_CONTEXT_RESOLVED",
            &format!(
                "gameId={game_id} source={} emulator={} emulatorInstallationId={emulator_installation_id}",
                launch_game.source,
                launch_game.emulator_id.as_deref().unwrap_or("none")
            ),
        );
        if let Some(steam_app_id) = launch_game.steam_app_id {
            self.set_steam_app_id(&session_id, steam_app_id);
        }
        session_log(
            &database,
            &session_id,
            "PLAY_REQUESTED",
            &format!(
                "gameId={game_id} source={} provider={} platform={} emulator={} titleId={}",
                launch_game.source,
                launch_game.provider,
                launch_game.platform,
                launch_game.emulator_id.as_deref().unwrap_or("none"),
                launch_game.title_id.as_deref().unwrap_or("none")
            ),
        );
        if launch_game.source.eq_ignore_ascii_case("emulator")
            && launch_game
                .emulator_id
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case("eden"))
        {
            self.run_eden(app, session_id, game_id, launch_game);
            return;
        }
        self.run_steam(
            app,
            session_id,
            game_id,
            launch_game.steam_app_id.unwrap_or_default(),
        );
    }

    fn run_eden(
        &self,
        app: AppHandle,
        session_id: String,
        game_id: String,
        launch_game: settings::LaunchGame,
    ) {
        let database = app.state::<settings::DatabaseState>();
        let Some(game_path) = launch_game.game_path.as_deref() else {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "Este juego no tiene un archivo de Eden disponible.".to_string(),
                Some("game-file-missing".to_string()),
            );
            return;
        };
        if !launch_game.installed {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "Este juego no está instalado localmente.".to_string(),
                Some("game-not-installed".to_string()),
            );
            return;
        }
        let target = match eden::resolve_launch_target(&database, game_path) {
            Ok(target) => target,
            Err(error) => {
                database.log(
                    "game-session",
                    "EDEN_EXECUTABLE_VALIDATION_FAILED",
                    &eden_telemetry_details(
                        &game_id,
                        launch_game.title_id.as_deref(),
                        "executable_validation",
                        "failure",
                    ),
                );
                let (state, reason) = match error {
                    eden::EdenError::ExecutableMissing => {
                        (GameSessionState::Unsupported, "eden-executable-missing")
                    }
                    eden::EdenError::GameMissing => {
                        (GameSessionState::Unsupported, "game-file-missing")
                    }
                    eden::EdenError::GameOutsideLibrary => (
                        GameSessionState::Unsupported,
                        "game-outside-configured-library",
                    ),
                    eden::EdenError::UnsupportedGame => {
                        (GameSessionState::Unsupported, "unsupported-game-file")
                    }
                    _ => (GameSessionState::Error, "eden-target-resolution-failed"),
                };
                self.fail(
                    &app,
                    &session_id,
                    state,
                    format!("No se pudo preparar el lanzamiento de Eden: {error}."),
                    Some(reason.to_string()),
                );
                return;
            }
        };
        if let Err(error) =
            self.apply_display_profile(&app, &session_id, &game_id, Some(&target.executable_path))
        {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Error,
                display_profile_failure_message(&error),
                Some(format!("display-profile-apply:{error}")),
            );
            return;
        }
        database.log(
            "game-session",
            "EDEN_EXECUTABLE_VALIDATED",
            &eden_telemetry_details(
                &game_id,
                launch_game.title_id.as_deref(),
                "executable_validation",
                "success",
            ),
        );
        database.log(
            "game-session",
            "EDEN_LAUNCH_COMMAND_PREPARED",
            &eden_telemetry_details(
                &game_id,
                launch_game.title_id.as_deref(),
                "launch_command_prepared",
                "success args=-f,-g",
            ),
        );
        self.set_state(
            &app,
            &session_id,
            GameSessionState::Launching,
            "Eden está preparando el juego…".to_string(),
            None,
        );

        let baseline_processes = snapshot_processes_for_executable(&target.executable_path);
        let baseline_pids: HashSet<u32> = baseline_processes
            .iter()
            .map(|process| process.pid)
            .collect();
        if !baseline_pids.is_empty() {
            database.log(
                "game-session",
                "EDEN_INSTANCE_ALREADY_RUNNING",
                &eden_telemetry_details(
                    &game_id,
                    launch_game.title_id.as_deref(),
                    "eden_instance_already_running",
                    "true",
                ),
            );
        }
        let mut child = match Command::new(&target.executable_path)
            .arg("-f")
            .arg("-g")
            .arg(&target.game_path)
            .current_dir(
                target
                    .executable_path
                    .parent()
                    .unwrap_or_else(|| Path::new(".")),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "No se pudo iniciar Eden.".to_string(),
                    Some(format!("eden-launch:{error}")),
                );
                return;
            }
        };
        let spawned_pid = child.id();
        database.log(
            "game-session",
            "EDEN_PROCESS_SPAWNED",
            &eden_telemetry_details(
                &game_id,
                launch_game.title_id.as_deref(),
                "process_spawned",
                &format!("success pid={spawned_pid}"),
            ),
        );
        database.log(
            "game-session",
            "EDEN_PROCESS_STARTED",
            &eden_telemetry_details(
                &game_id,
                launch_game.title_id.as_deref(),
                "process_started",
                "success",
            ),
        );

        let deadline = Instant::now() + Duration::from_secs(120);
        let mut state = GameSessionState::Launching;
        let mut active_checks = 0u8;
        let mut finishing_since: Option<Instant> = None;
        let mut child_exit_logged = false;
        let mut child_failed = false;
        let mut detected_logged = false;
        let mut process_alive_without_game_logged = false;
        let mut shortcut_monitor = GamepadShortcutMonitor::default();
        let mut gamepad_monitor_running = false;
        let mut stop_requested = false;
        let mut stop_forced = false;
        let mut stop_reason = "emulator_exit";
        let mut eden_playtime_before = None;
        loop {
            if !child_exit_logged {
                if let Ok(Some(status)) = child.try_wait() {
                    child_exit_logged = true;
                    child_failed = !status.success();
                    database.log(
                        "game-session",
                        "EDEN_EMULATOR_EXITED",
                        &eden_telemetry_details(
                            &game_id,
                            launch_game.title_id.as_deref(),
                            "emulator_exited",
                            if status.success() {
                                "success"
                            } else {
                                "failure"
                            },
                        ),
                    );
                }
            }
            let probe = inspect_eden_game_processes(&target.executable_path, &target.game_path);
            let detected_pids: HashSet<u32> = probe
                .processes
                .iter()
                .filter(|process| {
                    !baseline_pids.contains(&process.pid)
                        || probe.game_argument_pids.contains(&process.pid)
                })
                .map(|process| process.pid)
                .collect();
            self.update_tracked_processes(&database, &session_id, detected_pids);
            let new_process = probe
                .processes
                .iter()
                .any(|process| !baseline_pids.contains(&process.pid));
            if !probe.processes.is_empty()
                && !probe.game_argument_seen
                && !process_alive_without_game_logged
            {
                process_alive_without_game_logged = true;
                database.log(
                    "game-session",
                    "EDEN_PROCESS_ALIVE_GAME_NOT_DETECTED",
                    &eden_telemetry_details(
                        &game_id,
                        launch_game.title_id.as_deref(),
                        "emulator_process_alive",
                        "game_session_not_detected",
                    ),
                );
            }
            let active = eden_game_is_active(
                state,
                probe.game_argument_seen,
                new_process,
                child_exit_logged,
            );
            if active && !detected_logged {
                detected_logged = true;
                database.log(
                    "game-session",
                    if probe.game_argument_seen {
                        "EDEN_HANDOFF_PROCESS_DETECTED"
                    } else {
                        "EDEN_GAME_PROCESS_DETECTED"
                    },
                    &eden_telemetry_details(
                        &game_id,
                        launch_game.title_id.as_deref(),
                        if probe.game_argument_seen {
                            "delegated_handoff_process_detected"
                        } else {
                            "game_process_detected"
                        },
                        "success",
                    ),
                );
                database.log(
                    "game-session",
                    "EDEN_GAME_DETECTED",
                    &eden_telemetry_details(
                        &game_id,
                        launch_game.title_id.as_deref(),
                        "game_detected",
                        "success",
                    ),
                );
            }
            if state == GameSessionState::Running {
                if !gamepad_monitor_running {
                    gamepad_monitor_running = true;
                    session_log(
                        &database,
                        &session_id,
                        "GAMEPAD_MONITOR_STARTED",
                        "emulator=eden controller_mode=xinput",
                    );
                }
                let shortcut_events = shortcut_monitor.poll(Instant::now());
                for event in &shortcut_events {
                    log_shortcut_event(&database, &session_id, event);
                }
                if shortcut_events
                    .iter()
                    .any(|event| matches!(event, ShortcutEvent::Triggered { .. }))
                    && !stop_requested
                {
                    stop_requested = true;
                    stop_reason = "user_exit";
                    eden_playtime_before = eden::external_playtime_seconds(&database, &game_id)
                        .ok()
                        .flatten();
                    session_log(
                        &database,
                        &session_id,
                        "EDEN_PLAYTIME_SYNC_REQUESTED",
                        &format!(
                            "emulator=eden beforeSeconds={}",
                            eden_playtime_before
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        ),
                    );
                    session_log(
                        &database,
                        &session_id,
                        "SESSION_STOP_REQUESTED",
                        "requestSource=gamepad_shortcut emulator=eden",
                    );
                    let pids = self.tracked_processes(&session_id);
                    let outcome = self.stop_eden_processes(&database, &session_id, &pids);
                    stop_forced = outcome.forced;
                    if stop_forced {
                        stop_reason = "forced_exit";
                    }
                    finishing_since = Some(Instant::now() - Duration::from_secs(8));
                    self.set_state(
                        &app,
                        &session_id,
                        GameSessionState::Finishing,
                        "Cerrando el juego...".to_string(),
                        None,
                    );
                    state = GameSessionState::Finishing;
                }
            } else if gamepad_monitor_running {
                gamepad_monitor_running = false;
                session_log(
                    &database,
                    &session_id,
                    "GAMEPAD_MONITOR_STOPPED",
                    "emulator=eden",
                );
            }
            match state {
                GameSessionState::Launching => {
                    if active {
                        active_checks = active_checks.saturating_add(1);
                    } else {
                        active_checks = 0;
                    }
                    if active_checks >= 2 {
                        let activity_session_id =
                            match settings::start_game_session(&database, &game_id) {
                                Ok(id) => id,
                                Err(error) => {
                                    self.fail(
                                        &app,
                                        &session_id,
                                        GameSessionState::Error,
                                        "El juego inició, pero no se pudo registrar la actividad."
                                            .to_string(),
                                        Some(format!("activity-start:{error}")),
                                    );
                                    return;
                                }
                            };
                        self.mark_running(&session_id, timestamp_now(), activity_session_id);
                        session_log(
                            &database,
                            &session_id,
                            "SESSION_STATE_CHANGED",
                            "from=Launching to=Running",
                        );
                        database.log(
                            "game-session",
                            "EDEN_SESSION_STARTED",
                            &eden_telemetry_details(
                                &game_id,
                                launch_game.title_id.as_deref(),
                                "session_started",
                                "success",
                            ),
                        );
                        self.emit_current(&app);
                        state = GameSessionState::Running;
                    } else if Instant::now() >= deadline {
                        database.log(
                            "game-session",
                            "EDEN_LAUNCH_TIMEOUT",
                            &eden_telemetry_details(
                                &game_id,
                                launch_game.title_id.as_deref(),
                                "launch_timeout",
                                "failure",
                            ),
                        );
                        self.fail(
                            &app,
                            &session_id,
                            GameSessionState::Error,
                            "Eden recibió la solicitud, pero LumaDeck no pudo confirmar el inicio del juego.".to_string(),
                            Some("start-timeout".to_string()),
                        );
                        return;
                    }
                }
                GameSessionState::Running => {
                    if !active {
                        finishing_since = Some(Instant::now());
                        database.log(
                            "game-session",
                            "EDEN_SESSION_END_DETECTED",
                            &eden_telemetry_details(
                                &game_id,
                                launch_game.title_id.as_deref(),
                                "session_end_detected",
                                "success",
                            ),
                        );
                        self.set_state(
                            &app,
                            &session_id,
                            GameSessionState::Finishing,
                            "Esperando el cierre completo del juego.".to_string(),
                            None,
                        );
                        state = GameSessionState::Finishing;
                    }
                }
                GameSessionState::Finishing => {
                    if active && !stop_requested {
                        finishing_since = None;
                        self.set_state(
                            &app,
                            &session_id,
                            GameSessionState::Running,
                            "Juego iniciado".to_string(),
                            None,
                        );
                        state = GameSessionState::Running;
                    } else if finishing_since
                        .is_some_and(|started| started.elapsed() >= Duration::from_secs(8))
                    {
                        if child_failed {
                            database.log(
                                "game-session",
                                "EDEN_CRASH_DETECTED",
                                &eden_telemetry_details(
                                    &game_id,
                                    launch_game.title_id.as_deref(),
                                    "crash_detected",
                                    "failure",
                                ),
                            );
                        }
                        if let Err(error) = self.finish_activity(
                            &database,
                            &session_id,
                            &game_id,
                            child_failed || stop_forced,
                        ) {
                            self.fail(
                                &app,
                                &session_id,
                                GameSessionState::Error,
                                "La sesión terminó, pero no se pudo guardar su cierre.".to_string(),
                                Some(format!("activity-end:{error}")),
                            );
                            return;
                        }
                        database.log(
                            "game-session",
                            "EDEN_SESSION_ENDED",
                            &eden_telemetry_details(
                                &game_id,
                                launch_game.title_id.as_deref(),
                                "session_ended",
                                "success",
                            ),
                        );
                        session_log(
                            &database,
                            &session_id,
                            "SESSION_FINISHED",
                            &format!(
                                "gameId={game_id} durationSeconds={} reason={stop_reason}",
                                self.elapsed_seconds(&session_id)
                            ),
                        );
                        session_log(
                            &database,
                            &session_id,
                            "SESSION_STOP_COMPLETED",
                            &format!("exitMethod={stop_reason}"),
                        );
                        database.log(
                            "game-session",
                            "EDEN_PLAYTIME_PERSISTED",
                            &eden_telemetry_details(
                                &game_id,
                                launch_game.title_id.as_deref(),
                                "playtime_persisted",
                                "success once=true",
                            ),
                        );
                        database.log(
                            "game-session",
                            "EDEN_LAST_PLAYED_PERSISTED",
                            &eden_telemetry_details(
                                &game_id,
                                launch_game.title_id.as_deref(),
                                "lastPlayed_persisted",
                                "success",
                            ),
                        );
                        match eden::sync_playtime_after_session(&database) {
                            Ok(status) => {
                                let after = eden::external_playtime_seconds(&database, &game_id)
                                    .ok()
                                    .flatten();
                                session_log(
                                    &database,
                                    &session_id,
                                    "EDEN_PLAYTIME_SYNC_COMPLETED",
                                    &format!(
                                        "emulator=eden beforeSeconds={} afterSeconds={} synced={} unavailable={} reconciliation={}",
                                        eden_playtime_before
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "unknown".to_string()),
                                        after
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "unknown".to_string()),
                                        status.playtime_synced,
                                        status.playtime_unavailable,
                                        if eden_playtime_before == after {
                                            "unchanged"
                                        } else {
                                            "updated"
                                        }
                                    ),
                                );
                            }
                            Err(error) => session_log(
                                &database,
                                &session_id,
                                "EDEN_PLAYTIME_SYNC_ERROR",
                                &format!("emulator=eden error={error}"),
                            ),
                        }
                        self.finish(&app, &session_id);
                        return;
                    }
                }
                _ => return,
            }
            if matches!(
                state,
                GameSessionState::Running | GameSessionState::Finishing
            ) {
                self.emit_current(&app);
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn run_steam(
        &self,
        app: AppHandle,
        session_id: String,
        game_id: String,
        requested_app_id: i64,
    ) {
        let database = app.state::<settings::DatabaseState>();
        database.log(
            "game-session",
            "PLAY_REQUESTED",
            &format!("game_id={game_id} steam_app_id={requested_app_id}"),
        );

        if requested_app_id <= 0 {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "Este juego no tiene un Steam AppID válido.".to_string(),
                Some("invalid-steam-app-id".to_string()),
            );
            return;
        }

        let launch_game = match settings::get_steam_launch_game(&database, &game_id) {
            Ok(Some(game)) => game,
            Ok(None) => {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Unsupported,
                    "El juego no está disponible en la biblioteca local.".to_string(),
                    Some("game-not-found".to_string()),
                );
                return;
            }
            Err(error) => {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "No se pudo consultar la instalación del juego.".to_string(),
                    Some(format!("database: {error}")),
                );
                return;
            }
        };

        if !launch_game.provider.eq_ignore_ascii_case("steam")
            || !matches!(
                launch_game.platform.to_ascii_lowercase().as_str(),
                "pc" | "windows" | "steam"
            )
        {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "Este MVP solo admite juegos nativos de Steam.".to_string(),
                Some("provider-not-steam".to_string()),
            );
            return;
        }
        if launch_game.steam_app_id != Some(requested_app_id) {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "El Steam AppID del juego no coincide con su registro local.".to_string(),
                Some("steam-app-id-mismatch".to_string()),
            );
            return;
        }
        if !launch_game.installed {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "Este juego no está instalado localmente.".to_string(),
                Some("game-not-installed".to_string()),
            );
            return;
        }

        let Some(installation) = steam::resolve_steam_installation(requested_app_id) else {
            database.log(
                "game-session",
                "COMPATIBILITY_BLOCKED",
                &format!("game_id={game_id} rule=install-dir-unresolved"),
            );
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "No se pudo resolver la carpeta de instalación del juego.".to_string(),
                Some("install-dir-unresolved".to_string()),
            );
            return;
        };
        database.log(
            "game-session",
            "MANIFEST_FOUND",
            &format!(
                "game_id={game_id} manifest={} install_dir={}",
                installation.manifest_path.display(),
                installation.install_dir.display()
            ),
        );
        let display_executable =
            match resolve_rtx_hdr_executable(&database, &game_id, &installation.install_dir) {
                Ok(path) => path,
                Err(error) => {
                    self.fail(
                        &app,
                        &session_id,
                        GameSessionState::Error,
                        "No se pudo resolver el ejecutable para RTX HDR; el juego no se iniciará."
                            .to_string(),
                        Some(format!("display-profile-executable:{error}")),
                    );
                    return;
                }
            };
        if let Err(error) =
            self.apply_display_profile(&app, &session_id, &game_id, display_executable.as_deref())
        {
            self.fail(
                &app,
                &session_id,
                GameSessionState::Error,
                display_profile_failure_message(&error),
                Some(format!("display-profile-apply:{error}")),
            );
            return;
        }
        let assessment = inspect_compatibility(&installation.install_dir);
        if let Some(issue) = assessment.unsupported_issue.as_ref() {
            database.log(
                "game-session",
                "COMPATIBILITY_BLOCKED",
                &format!(
                    "game_id={game_id} kind={} rule={} path={}",
                    issue.kind,
                    issue.rule,
                    issue.path.display()
                ),
            );
            self.fail(
                &app,
                &session_id,
                GameSessionState::Unsupported,
                "Este juego utiliza un launcher secundario que todavía no está soportado."
                    .to_string(),
                Some(format!("{}:{}", issue.kind, issue.rule)),
            );
            return;
        }
        self.set_monitoring(
            &session_id,
            assessment.monitoring_mode,
            assessment.anti_cheat_provider.clone(),
            assessment.compatible_reason.clone(),
        );
        database.log(
            "game-session",
            "monitoring-mode-selected",
            &format!(
                "game_id={game_id} mode={} reason={}",
                assessment.monitoring_mode.as_str(),
                assessment
                    .anti_cheat_provider
                    .as_deref()
                    .or(assessment.compatible_reason.as_deref())
                    .unwrap_or("native-steam")
            ),
        );
        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea") {
            database.log(
                "game-session",
                "ea-secondary-launcher-detected",
                &format!(
                    "appId={requested_app_id} sessionId={session_id} game_id={game_id} reason=secondary-launcher-ea"
                ),
            );
        }
        database.log(
            "game-session",
            "COMPATIBILITY_ACCEPTED",
            &format!(
                "game_id={game_id} install_dir={}",
                installation.install_dir.display()
            ),
        );
        if assessment.monitoring_mode == MonitoringMode::Compatible {
            let compatible_message =
                if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea") {
                    "Steam está iniciando el juego mediante EA App.".to_string()
                } else {
                    format!(
                        "Modo de compatibilidad. Este juego utiliza {}.",
                        assessment
                            .anti_cheat_provider
                            .as_deref()
                            .unwrap_or("un sistema anti-cheat")
                    )
                };
            self.set_state(
                &app,
                &session_id,
                GameSessionState::Preparing,
                compatible_message,
                None,
            );
        }
        self.update_install_dir(&session_id, installation.install_dir.clone());
        let frame_generation_profile =
            match settings::get_frame_generation_profile(&database, &game_id) {
                Ok(profile) => profile,
                Err(error) => {
                    self.fail(
                        &app,
                        &session_id,
                        GameSessionState::Error,
                        "No se pudo leer el perfil de Frame Generation.".to_string(),
                        Some(format!("frame-generation-read:{error}")),
                    );
                    return;
                }
            };
        let frame_generation_target_known = frame_generation_profile
            .target_executable
            .as_deref()
            .is_some_and(|path| std::path::Path::new(path).is_file());
        database.log(
            "game-session",
            "FRAME_GENERATION_VALIDATED",
            &format!(
                "game_id={game_id} enabled={} target_known={} target={}",
                frame_generation_profile.enabled,
                frame_generation_target_known,
                frame_generation_profile
                    .target_executable
                    .as_deref()
                    .unwrap_or("<missing>")
            ),
        );
        if frame_generation_profile.enabled && frame_generation_target_known {
            let provider = LosslessScalingProvider;
            let sync = match provider.synchronize_if_needed(&frame_generation_profile) {
                Ok(sync) => sync,
                Err(error) => {
                    self.fail(
                        &app,
                        &session_id,
                        GameSessionState::Error,
                        "No se pudo sincronizar Frame Generation con Lossless Scaling.".to_string(),
                        Some(format!("frame-generation-sync:{error}")),
                    );
                    return;
                }
            };
            if sync.restart_required {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "Lossless Scaling necesita reiniciarse para aplicar el perfil.".to_string(),
                    Some("frame-generation-restart-required".to_string()),
                );
                return;
            }
        }
        if frame_generation_profile.enabled && frame_generation_target_known {
            let was_running = is_lossless_scaling_running();
            if !was_running {
                database.log(
                    "game-session",
                    "LOSSLESS_SCALING_START_REQUESTED",
                    &format!("game_id={game_id}"),
                );
                if let Err(error) = LosslessScalingProvider.ensure_running() {
                    database.log(
                        "game-session",
                        "LOSSLESS_SCALING_START_FAILED",
                        &format!("game_id={game_id} error={error}"),
                    );
                    self.fail(
                        &app,
                        &session_id,
                        GameSessionState::Error,
                        "No se pudo iniciar Lossless Scaling.".to_string(),
                        Some(format!("lossless-scaling-start:{error}")),
                    );
                    return;
                }
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stable_checks = 0;
            while Instant::now() < deadline {
                if is_lossless_scaling_running() {
                    stable_checks += 1;
                    if stable_checks >= 2 {
                        break;
                    }
                } else {
                    stable_checks = 0;
                }
                thread::sleep(Duration::from_millis(100));
            }
            if stable_checks < 2 {
                database.log(
                    "game-session",
                    "LOSSLESS_SCALING_START_TIMEOUT",
                    &format!("game_id={game_id}"),
                );
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "Lossless Scaling no estuvo listo a tiempo.".to_string(),
                    Some("lossless-scaling-start-timeout".to_string()),
                );
                return;
            }
            database.log(
                "game-session",
                "LOSSLESS_SCALING_READY",
                &format!("game_id={game_id} started_by_lumadeck={}", !was_running),
            );
        } else if frame_generation_profile.enabled {
            database.log(
                "game-session",
                "FRAME_GENERATION_DEFERRED",
                &format!(
                    "game_id={game_id} reason=target-executable-required target={}",
                    frame_generation_profile
                        .target_executable
                        .as_deref()
                        .unwrap_or("<missing>")
                ),
            );
        }
        self.set_state(
            &app,
            &session_id,
            GameSessionState::Launching,
            "Steam está preparando el juego…".to_string(),
            None,
        );

        let pre_launch_processes = snapshot_processes();
        if let Err(error) = launch_steam_app(requested_app_id) {
            database.log(
                "game-session",
                "ERROR",
                &format!("game_id={game_id} error=steam-launch:{error}"),
            );
            self.fail(
                &app,
                &session_id,
                GameSessionState::Error,
                "No se pudo enviar la solicitud de lanzamiento a Steam.".to_string(),
                Some("steam-launch-failed".to_string()),
            );
            return;
        }
        database.log(
            "game-session",
            "STEAM_URI_SENT",
            &format!("game_id={game_id} steam_app_id={requested_app_id}"),
        );
        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea") {
            database.log(
                "game-session",
                "ea-launch-chain-started",
                &format!("appId={requested_app_id} sessionId={session_id} game_id={game_id}"),
            );
        }

        let mut tracker = ProcessTracker::new_with_mode(
            installation.install_dir.clone(),
            pre_launch_processes.clone(),
            assessment.monitoring_mode,
        );
        let baseline_pids: HashSet<u32> = pre_launch_processes
            .iter()
            .map(|process| process.pid)
            .collect();
        let mut ea_launcher_seen = HashSet::new();
        let mut ea_anticheat_seen = HashSet::new();
        let mut frame_generation_target_learned = frame_generation_profile
            .target_executable
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty());
        let launch_deadline = Instant::now() + Duration::from_secs(120);
        let mut state = GameSessionState::Launching;
        let mut finishing_since: Option<Instant> = None;

        loop {
            let processes = snapshot_processes();
            let observation = tracker.observe(&processes);
            self.update_tracked_processes(&database, &session_id, observation.confirmed_pids());
            if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea") {
                for process in processes.iter().filter(|process| {
                    !baseline_pids.contains(&process.pid) && is_ea_launcher_process(process)
                }) {
                    if ea_launcher_seen.insert(process.pid) {
                        database.log(
                            "game-session",
                            "ea-launcher-seen",
                            &format!(
                                "appId={requested_app_id} sessionId={session_id} pid={} process={} executable={}",
                                process.pid,
                                process_name(process),
                                process.path.display()
                            ),
                        );
                    }
                }
                for process in processes.iter().filter(|process| {
                    !baseline_pids.contains(&process.pid) && is_ea_anticheat_process(process)
                }) {
                    if ea_anticheat_seen.insert(process.pid) {
                        database.log(
                            "game-session",
                            "ea-anticheat-seen",
                            &format!(
                                "appId={requested_app_id} sessionId={session_id} pid={} process={} executable={}",
                                process.pid,
                                process_name(process),
                                process.path.display()
                            ),
                        );
                    }
                }
            }
            if assessment.monitoring_mode == MonitoringMode::Compatible {
                for process in &observation.new_candidates {
                    database.log(
                        "game-session",
                        "compatible-process-candidate",
                        &format!(
                            "game_id={game_id} pid={} path={}",
                            process.pid,
                            process.path.display()
                        ),
                    );
                    if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea") {
                        database.log(
                            "game-session",
                            "ea-game-candidate",
                            &format!(
                                "appId={requested_app_id} sessionId={session_id} pid={} process={} executable={}",
                                process.pid,
                                process_name(process),
                                process.path.display()
                            ),
                        );
                    }
                }
            }
            for process in &observation.new_confirmed {
                database.log(
                    "game-session",
                    "PROCESS_CONFIRMED",
                    &format!(
                        "game_id={game_id} pid={} path={}",
                        process.pid,
                        process.path.display()
                    ),
                );
                if assessment.monitoring_mode == MonitoringMode::Compatible {
                    database.log(
                        "game-session",
                        "compatible-process-confirmed",
                        &format!(
                            "game_id={game_id} pid={} path={}",
                            process.pid,
                            process.path.display()
                        ),
                    );
                }
                if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea") {
                    database.log(
                        "game-session",
                        "ea-game-confirmed",
                        &format!(
                            "appId={requested_app_id} sessionId={session_id} pid={} process={} executable={}",
                            process.pid,
                            process_name(process),
                            process.path.display()
                        ),
                    );
                }
                if !frame_generation_target_learned
                    && process.path.is_file()
                    && settings::set_frame_generation_target(
                        &database,
                        &game_id,
                        &process.path.to_string_lossy(),
                    )
                    .is_ok()
                {
                    frame_generation_target_learned = true;
                    database.log(
                        "game-session",
                        "FRAME_GENERATION_TARGET_LEARNED",
                        &format!(
                            "game_id={game_id} target_executable={}",
                            process.path.display()
                        ),
                    );
                }
            }

            match state {
                GameSessionState::Launching => {
                    if !observation.confirmed_alive.is_empty() {
                        let activity_session_id =
                            match settings::start_game_session(&database, &game_id) {
                                Ok(id) => id,
                                Err(error) => {
                                    self.fail(
                                        &app,
                                        &session_id,
                                        GameSessionState::Error,
                                        "El juego inició, pero no se pudo registrar la actividad."
                                            .to_string(),
                                        Some(format!("activity-start:{error}")),
                                    );
                                    return;
                                }
                            };
                        let started_at = timestamp_now();
                        self.mark_running(&session_id, started_at.clone(), activity_session_id);
                        if frame_generation_profile.enabled && !frame_generation_target_known {
                            self.set_state(
                                &app,
                                &session_id,
                                GameSessionState::Running,
                                "Juego iniciado. Frame Generation estará preparada para el próximo lanzamiento.".to_string(),
                                None,
                            );
                        }
                        database.log(
                            "game-session",
                            "RUNNING",
                            &format!("game_id={game_id} activity_session_id={activity_session_id}"),
                        );
                        if assessment.monitoring_mode == MonitoringMode::Compatible {
                            database.log(
                                "game-session",
                                "compatible-running",
                                &format!("game_id={game_id}"),
                            );
                        }
                        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea")
                        {
                            database.log(
                                "game-session",
                                "ea-compatible-running",
                                &format!(
                                    "appId={requested_app_id} sessionId={session_id} game_id={game_id}"
                                ),
                            );
                        }
                        self.emit_current(&app);
                        state = GameSessionState::Running;
                    } else if Instant::now() >= launch_deadline {
                        database.log(
                            "game-session",
                            "TIMEOUT",
                            &format!("game_id={game_id} timeout_seconds=120"),
                        );
                        if assessment.monitoring_mode == MonitoringMode::Compatible {
                            database.log(
                                "game-session",
                                "compatible-timeout",
                                &format!("game_id={game_id}"),
                            );
                        }
                        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea")
                        {
                            database.log(
                                "game-session",
                                "ea-compatible-timeout",
                                &format!(
                                    "appId={requested_app_id} sessionId={session_id} game_id={game_id}"
                                ),
                            );
                        }
                        let timeout_message = if assessment.compatible_reason.as_deref()
                            == Some("secondary-launcher-ea")
                        {
                            "Steam y EA iniciaron el proceso, pero LumaDeck no pudo confirmar el inicio del juego."
                        } else if assessment.monitoring_mode == MonitoringMode::Compatible {
                            "Steam inició el proceso, pero LumaDeck no pudo confirmar de forma segura que el juego esté ejecutándose."
                        } else {
                            "Steam recibió la solicitud, pero LumaDeck no pudo confirmar el inicio del juego."
                        };
                        self.fail(
                            &app,
                            &session_id,
                            GameSessionState::Error,
                            timeout_message.to_string(),
                            Some("start-timeout".to_string()),
                        );
                        return;
                    }
                }
                GameSessionState::Running => {
                    if observation.confirmed_alive.is_empty() {
                        finishing_since = Some(Instant::now());
                        database.log(
                            "game-session",
                            "PROCESS_DISAPPEARED",
                            &format!("game_id={game_id}"),
                        );
                        if assessment.monitoring_mode == MonitoringMode::Compatible {
                            database.log(
                                "game-session",
                                "compatible-process-lost",
                                &format!("game_id={game_id}"),
                            );
                        }
                        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea")
                        {
                            database.log(
                                "game-session",
                                "ea-game-process-lost",
                                &format!(
                                    "appId={requested_app_id} sessionId={session_id} game_id={game_id}"
                                ),
                            );
                        }
                        self.set_state(
                            &app,
                            &session_id,
                            GameSessionState::Finishing,
                            "Esperando el cierre completo del juego.".to_string(),
                            None,
                        );
                        state = GameSessionState::Finishing;
                    }
                }
                GameSessionState::Finishing => {
                    if !observation.confirmed_alive.is_empty() {
                        database.log(
                            "game-session",
                            "PROCESS_REPLACEMENT",
                            &format!("game_id={game_id}"),
                        );
                        if assessment.monitoring_mode == MonitoringMode::Compatible {
                            database.log(
                                "game-session",
                                "compatible-process-replacement",
                                &format!("game_id={game_id}"),
                            );
                        }
                        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea")
                        {
                            database.log(
                                "game-session",
                                "ea-game-process-replacement",
                                &format!(
                                    "appId={requested_app_id} sessionId={session_id} game_id={game_id}"
                                ),
                            );
                        }
                        self.set_state(
                            &app,
                            &session_id,
                            GameSessionState::Running,
                            "Juego iniciado".to_string(),
                            None,
                        );
                        state = GameSessionState::Running;
                        finishing_since = None;
                    } else if finishing_since
                        .is_some_and(|started| started.elapsed() >= Duration::from_secs(8))
                    {
                        if let Err(error) =
                            self.finish_activity(&database, &session_id, &game_id, false)
                        {
                            self.fail(
                                &app,
                                &session_id,
                                GameSessionState::Error,
                                "La sesión terminó, pero no se pudo guardar su cierre.".to_string(),
                                Some(format!("activity-end:{error}")),
                            );
                            return;
                        }
                        if let Err(error) = self.restore_display(&app, &session_id, false) {
                            self.set_state(
                                &app,
                                &session_id,
                                GameSessionState::Error,
                                "La sesion termino, pero no se pudo restaurar la pantalla."
                                    .to_string(),
                                Some(format!("display-restore:{error}")),
                            );
                            return;
                        }
                        database.log(
                            "game-session",
                            "SESSION_FINISHED",
                            &format!("game_id={game_id}"),
                        );
                        if assessment.monitoring_mode == MonitoringMode::Compatible {
                            database.log(
                                "game-session",
                                "compatible-session-finished",
                                &format!("game_id={game_id}"),
                            );
                        }
                        if assessment.compatible_reason.as_deref() == Some("secondary-launcher-ea")
                        {
                            database.log(
                                "game-session",
                                "ea-compatible-finished",
                                &format!(
                                    "appId={requested_app_id} sessionId={session_id} game_id={game_id}"
                                ),
                            );
                        }
                        self.finish(&app, &session_id);
                        return;
                    }
                }
                _ => return,
            }
            if matches!(
                state,
                GameSessionState::Running | GameSessionState::Finishing
            ) {
                self.emit_current(&app);
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn update_install_dir(&self, session_id: &str, install_dir: PathBuf) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.install_dir = Some(install_dir);
            }
        }
    }

    fn set_monitoring(
        &self,
        session_id: &str,
        monitoring_mode: MonitoringMode,
        anti_cheat_provider: Option<String>,
        compatible_reason: Option<String>,
    ) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.monitoring_mode = monitoring_mode;
                session.anti_cheat_provider = anti_cheat_provider;
                session.compatible_reason = compatible_reason;
                session.capabilities = SessionCapabilities::for_mode(monitoring_mode);
            }
        }
    }

    fn update_tracked_processes(
        &self,
        database: &settings::DatabaseState,
        session_id: &str,
        pids: HashSet<u32>,
    ) {
        let (attached, detached) = {
            let mut current = match self.active.lock() {
                Ok(current) => current,
                Err(_) => return,
            };
            let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            else {
                return;
            };
            let attached = pids
                .difference(&session.tracked_processes)
                .copied()
                .collect::<Vec<_>>();
            let detached = session
                .tracked_processes
                .difference(&pids)
                .copied()
                .collect::<Vec<_>>();
            session.tracked_processes = pids;
            (attached, detached)
        };
        for pid in attached {
            session_log(
                database,
                session_id,
                "PROCESS_PID_ATTACHED",
                &format!("pid={pid}"),
            );
        }
        for pid in detached {
            session_log(
                database,
                session_id,
                "PROCESS_PID_DETACHED",
                &format!("pid={pid}"),
            );
        }
    }

    fn tracked_processes(&self, session_id: &str) -> HashSet<u32> {
        if let Ok(current) = self.active.lock() {
            if let Some(session) = current
                .as_ref()
                .filter(|session| session.session_id == session_id)
            {
                return session.tracked_processes.clone();
            }
        }
        HashSet::new()
    }

    fn elapsed_seconds(&self, session_id: &str) -> i64 {
        self.active
            .lock()
            .ok()
            .and_then(|current| {
                current
                    .as_ref()
                    .filter(|session| session.session_id == session_id)
                    .and_then(|session| session.started_at.as_deref())
                    .map(duration_since_timestamp)
            })
            .unwrap_or_default()
    }

    fn stop_eden_processes(
        &self,
        database: &settings::DatabaseState,
        session_id: &str,
        pids: &HashSet<u32>,
    ) -> StopOutcome {
        let mut target_pids = pids.iter().copied().collect::<Vec<_>>();
        target_pids.sort_unstable();
        if target_pids.is_empty() {
            session_log_level(
                database,
                session_id,
                "PROCESS_LOOKUP_ERROR",
                "ERROR",
                "emulator=eden error=no_session_pids",
            );
            return StopOutcome { forced: false };
        }

        session_log(
            database,
            session_id,
            "EMULATOR_GRACEFUL_CLOSE_STARTED",
            &format!("emulator=eden pidCount={}", target_pids.len()),
        );
        let mut graceful_requested = false;
        for pid in &target_pids {
            session_log(
                database,
                session_id,
                "EMULATOR_GRACEFUL_CLOSE_REQUESTED",
                &format!("emulator=eden pid={pid}"),
            );
            match request_window_close(*pid) {
                Ok(true) => {
                    graceful_requested = true;
                    session_log(
                        database,
                        session_id,
                        "EMULATOR_GRACEFUL_CLOSE_RESULT",
                        &format!("emulator=eden pid={pid} result=window_close_sent"),
                    );
                }
                Ok(false) => session_log(
                    database,
                    session_id,
                    "EMULATOR_GRACEFUL_CLOSE_RESULT",
                    &format!("emulator=eden pid={pid} result=no_window"),
                ),
                Err(error) => session_log_level(
                    database,
                    session_id,
                    "EMULATOR_GRACEFUL_CLOSE_ERROR",
                    "ERROR",
                    &format!("emulator=eden pid={pid} error={error}"),
                ),
            }
        }

        let graceful_deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < graceful_deadline
            && target_pids.iter().any(|pid| process_is_alive(*pid))
        {
            thread::sleep(Duration::from_millis(250));
        }
        if !target_pids.iter().any(|pid| process_is_alive(*pid)) {
            session_log(
                database,
                session_id,
                "EMULATOR_GRACEFUL_CLOSE_SUCCEEDED",
                &format!("emulator=eden result=processes_exited requested={graceful_requested}"),
            );
            return StopOutcome { forced: false };
        }
        session_log_level(
            database,
            session_id,
            "EMULATOR_GRACEFUL_CLOSE_TIMEOUT",
            "WARN",
            "emulator=eden timeoutMs=4000",
        );

        let mut force_attempted = false;
        for pid in target_pids
            .iter()
            .copied()
            .filter(|pid| process_is_alive(*pid))
        {
            force_attempted = true;
            session_log(
                database,
                session_id,
                "EMULATOR_FORCE_TERMINATE_REQUESTED",
                &format!("emulator=eden pid={pid}"),
            );
            match force_terminate_process(pid) {
                Ok(()) => session_log(
                    database,
                    session_id,
                    "EMULATOR_FORCE_TERMINATE_SUCCEEDED",
                    &format!("emulator=eden pid={pid}"),
                ),
                Err(error) => session_log_level(
                    database,
                    session_id,
                    "EMULATOR_FORCE_TERMINATE_ERROR",
                    "ERROR",
                    &format!("emulator=eden pid={pid} error={error}"),
                ),
            }
        }
        StopOutcome {
            forced: force_attempted,
        }
    }

    fn restore_display(
        &self,
        app: &AppHandle,
        session_id: &str,
        _force: bool,
    ) -> Result<(), String> {
        let database = app.state::<settings::DatabaseState>();
        launch_display::restore(&database, Some(session_id))
    }

    fn apply_display_profile(
        &self,
        app: &AppHandle,
        session_id: &str,
        game_id: &str,
        executable: Option<&Path>,
    ) -> Result<(), String> {
        let database = app.state::<settings::DatabaseState>();
        let display_profile = settings::get_display_profile(&database, game_id)
            .map_err(|error| format!("display-profile-read:{error}"))?;
        let recommendation =
            match launch_display::resolve_cached_hdr_recommendation(&database, &display_profile) {
                Ok(recommendation) => recommendation,
                Err(error) => {
                    session_log_level(
                        &database,
                        session_id,
                        "DISPLAY_PROFILE_AUTO_HDR_SKIPPED",
                        "WARN",
                        &format!("gameId={game_id};error={error}"),
                    );
                    None
                }
            };
        let result = launch_display::apply_profile(
            &database,
            session_id,
            game_id,
            &display_profile,
            recommendation,
            executable,
        )?;
        for warning in result.warnings {
            session_log_level(
                &database,
                session_id,
                "DISPLAY_PROFILE_WARNING",
                "WARN",
                &format!("gameId={game_id};warning={warning}"),
            );
        }
        Ok(())
    }

    fn mark_running(&self, session_id: &str, started_at: String, activity_session_id: i64) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.state = GameSessionState::Running;
                session.started_at = Some(started_at);
                session.activity_session_id = Some(activity_session_id);
                session.message = "Juego iniciado".to_string();
            }
        }
    }

    fn set_source(&self, session_id: &str, source: String) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.source = source;
            }
        }
    }

    fn set_steam_app_id(&self, session_id: &str, steam_app_id: i64) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.steam_app_id = steam_app_id;
            }
        }
    }

    fn finish_activity(
        &self,
        database: &settings::DatabaseState,
        session_id: &str,
        game_id: &str,
        interrupted: bool,
    ) -> Result<(), String> {
        let (activity_session_id, already_ended) = self
            .active
            .lock()
            .ok()
            .and_then(|current| {
                current
                    .as_ref()
                    .filter(|session| session.session_id == session_id)
                    .map(|session| (session.activity_session_id, session.activity_ended))
            })
            .ok_or_else(|| "missing-activity-session-id".to_string())?;
        if already_ended {
            return Ok(());
        }
        let activity_session_id =
            activity_session_id.ok_or_else(|| "missing-activity-session-id".to_string())?;
        settings::end_game_session(database, game_id, activity_session_id, interrupted)
            .map_err(|error| error.to_string())?;
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.activity_ended = true;
            }
        }
        Ok(())
    }

    fn set_state(
        &self,
        app: &AppHandle,
        session_id: &str,
        state: GameSessionState,
        message: String,
        unsupported_reason: Option<String>,
    ) {
        let transition = if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                let previous = session.state;
                session.state = state;
                session.message = message;
                session.unsupported_reason = unsupported_reason;
                Some(previous)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(previous) = transition.filter(|previous| *previous != state) {
            let database = app.state::<settings::DatabaseState>();
            session_log(
                &database,
                session_id,
                "SESSION_STATE_CHANGED",
                &format!("from={previous:?} to={state:?}"),
            );
        }
        self.emit_current(app);
    }

    fn fail(
        &self,
        app: &AppHandle,
        session_id: &str,
        state: GameSessionState,
        message: String,
        unsupported_reason: Option<String>,
    ) {
        let message = match self.restore_display(app, session_id, true) {
            Ok(()) => message,
            Err(error) => format!("{message} No se pudo restaurar la pantalla: {error}"),
        };
        self.set_state(app, session_id, state, message, unsupported_reason);
    }

    fn emit_current(&self, app: &AppHandle) {
        let status = self.current_status();
        emit_status(app, &status);
    }

    fn finish(&self, app: &AppHandle, session_id: &str) {
        let status = if let Ok(mut current) = self.active.lock() {
            let Some(session) = current
                .as_ref()
                .filter(|session| session.session_id == session_id)
            else {
                return;
            };
            let mut final_status = status_from_session(session);
            final_status.state = GameSessionState::Idle;
            final_status.occurred_at = timestamp_now();
            final_status.elapsed_seconds = session
                .started_at
                .as_deref()
                .map(duration_since_timestamp)
                .unwrap_or_default();
            final_status.message = "Sesión finalizada".to_string();
            *current = None;
            final_status
        } else {
            return;
        };
        emit_status(app, &status);
        let database = app.state::<settings::DatabaseState>();
        session_log(&database, session_id, "READY_RESTORED", "state=ready");
    }
}

fn is_blocking_state(state: GameSessionState) -> bool {
    matches!(
        state,
        GameSessionState::Preparing
            | GameSessionState::Launching
            | GameSessionState::Running
            | GameSessionState::Finishing
    )
}

#[derive(Debug, Default)]
struct StopOutcome {
    forced: bool,
}

fn session_log(
    database: &settings::DatabaseState,
    session_id: &str,
    checkpoint: &str,
    details: &str,
) {
    session_log_level(database, session_id, checkpoint, "INFO", details);
}

fn session_log_level(
    database: &settings::DatabaseState,
    session_id: &str,
    checkpoint: &str,
    level: &str,
    details: &str,
) {
    database.log(
        "game-session",
        checkpoint,
        &format!(
            "level={level} sessionId={} {details}",
            sanitize_id(session_id)
        ),
    );
}

fn log_shortcut_event(database: &settings::DatabaseState, session_id: &str, event: &ShortcutEvent) {
    let (checkpoint, level, details) = match event {
        ShortcutEvent::ControllerConnected { controller_index } => (
            "GAMEPAD_CONTROLLER_CONNECTED",
            "DEBUG",
            format!("controllerIndex={controller_index}"),
        ),
        ShortcutEvent::ControllerDisconnected { controller_index } => (
            "GAMEPAD_CONTROLLER_DISCONNECTED",
            "DEBUG",
            format!("controllerIndex={controller_index}"),
        ),
        ShortcutEvent::HoldStarted { controller_index } => (
            "GAMEPAD_SHORTCUT_HOLD_STARTED",
            "DEBUG",
            format!("controllerIndex={controller_index} shortcut=Start+Select"),
        ),
        ShortcutEvent::HoldCancelled { controller_index } => (
            "GAMEPAD_SHORTCUT_HOLD_CANCELLED",
            "DEBUG",
            format!("controllerIndex={controller_index} shortcut=Start+Select"),
        ),
        ShortcutEvent::ThresholdReached {
            controller_index,
            hold_ms,
        } => (
            "GAMEPAD_SHORTCUT_THRESHOLD_REACHED",
            "INFO",
            format!("controllerIndex={controller_index} holdMs={hold_ms}"),
        ),
        ShortcutEvent::Triggered {
            controller_index,
            hold_ms,
        } => (
            "GAMEPAD_SHORTCUT_TRIGGERED",
            "INFO",
            format!("controllerIndex={controller_index} holdMs={hold_ms} action=StopGameSession"),
        ),
        ShortcutEvent::DuplicateTriggerIgnored { controller_index } => (
            "GAMEPAD_SHORTCUT_DUPLICATE_TRIGGER_IGNORED",
            "DEBUG",
            format!("controllerIndex={controller_index}"),
        ),
        ShortcutEvent::MonitorError { code } => (
            "GAMEPAD_MONITOR_ERROR",
            "ERROR",
            format!("controllerApi=XInput code={code}"),
        ),
    };
    database.log(
        "game-session",
        checkpoint,
        &format!(
            "level={level} sessionId={} {details}",
            sanitize_id(session_id)
        ),
    );
}

#[cfg(windows)]
fn request_window_close(pid: u32) -> Result<bool, String> {
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
    Ok(context.1)
}

#[cfg(not(windows))]
fn request_window_close(_pid: u32) -> Result<bool, String> {
    Err("graceful close is only available on Windows".to_string())
}

fn process_is_alive(pid: u32) -> bool {
    snapshot_processes()
        .iter()
        .any(|process| process.pid == pid)
}

#[cfg(windows)]
fn force_terminate_process(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return Err(format!("OpenProcess failed for pid={pid}"));
        }
        let result = TerminateProcess(handle, 1);
        CloseHandle(handle);
        if result == 0 {
            return Err(format!("TerminateProcess failed for pid={pid}"));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn force_terminate_process(_pid: u32) -> Result<(), String> {
    Err("force termination is only available on Windows".to_string())
}

fn status_from_session(session: &ActiveSession) -> GameSessionStatus {
    GameSessionStatus {
        session_id: session.session_id.clone(),
        game_id: session.game_id.clone(),
        steam_app_id: session.steam_app_id,
        source: session.source.clone(),
        state: session.state,
        occurred_at: timestamp_now(),
        elapsed_seconds: session
            .started_at
            .as_deref()
            .map(duration_since_timestamp)
            .unwrap_or_default(),
        message: session.message.clone(),
        unsupported_reason: session.unsupported_reason.clone(),
        monitoring_mode: session.monitoring_mode,
        anti_cheat_provider: session.anti_cheat_provider.clone(),
        compatible_reason: session.compatible_reason.clone(),
        capabilities: session.capabilities,
    }
}

fn idle_status() -> GameSessionStatus {
    GameSessionStatus {
        session_id: String::new(),
        game_id: String::new(),
        steam_app_id: 0,
        source: "none".to_string(),
        state: GameSessionState::Idle,
        occurred_at: timestamp_now(),
        elapsed_seconds: 0,
        message: String::new(),
        unsupported_reason: None,
        monitoring_mode: MonitoringMode::Full,
        anti_cheat_provider: None,
        compatible_reason: None,
        capabilities: SessionCapabilities::for_mode(MonitoringMode::Full),
    }
}

fn emit_status(app: &AppHandle, status: &GameSessionStatus) {
    let _ = app.emit(GAME_SESSION_EVENT, status);
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn timestamp_now() -> String {
    unix_seconds().to_string()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn duration_since_timestamp(value: &str) -> i64 {
    unix_seconds().saturating_sub(value.parse::<u64>().unwrap_or(unix_seconds())) as i64
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompatibilityIssue {
    kind: &'static str,
    rule: &'static str,
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompatibilityAssessment {
    monitoring_mode: MonitoringMode,
    anti_cheat_provider: Option<String>,
    compatible_reason: Option<String>,
    unsupported_issue: Option<CompatibilityIssue>,
}

impl MonitoringMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compatible => "compatible",
        }
    }
}

const ANTI_CHEAT_PATTERNS: &[&str] = &[
    "easyanticheat_eos",
    "easyanticheat",
    "start_protected_game",
    "battleye",
    "beservice",
    "beclient",
    "eaanticheat.gameservice",
    "eaanticheat",
    "pnkbstr",
    "gameguard",
    "xigncode",
];

const ANTI_CHEAT_PROVIDER_NAMES: &[(&str, &str)] = &[
    ("easyanticheat_eos", "easy-anticheat-eos"),
    ("easyanticheat", "easy-anticheat"),
    ("start_protected_game", "easy-anticheat"),
    ("battleye", "battleye"),
    ("beservice", "battleye"),
    ("beclient", "battleye"),
    ("eaanticheat.gameservice", "ea-anticheat"),
    ("eaanticheat", "ea-anticheat"),
    ("pnkbstr", "punkbuster"),
    ("gameguard", "nprotect-gameguard"),
    ("xigncode", "xigncode"),
];

const SECONDARY_LAUNCHER_PATTERNS: &[&str] = &[
    "ubisoftconnect",
    "ubisoftgamelauncher",
    "uplay",
    "rockstar games launcher",
    "socialclub",
    "battle.net",
    "epicgameslauncher",
    "epic games launcher",
];

const EA_SECONDARY_LAUNCHER_PATTERNS: &[&str] = &[
    "originwebhelperservice",
    "originclientservice",
    "originclient",
    "origin",
    "eabackgroundservice",
    "eadesktop",
    "ea desktop",
    "ea app",
    "eaapp",
    "ealauncher",
];

fn inspect_compatibility(install_dir: &Path) -> CompatibilityAssessment {
    let mut pending = vec![(install_dir.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    let mut anti_cheat: Option<(&'static str, PathBuf)> = None;
    let mut ea_secondary_launcher: Option<(&'static str, PathBuf)> = None;
    let mut secondary_launcher: Option<(&'static str, PathBuf)> = None;
    while let Some((path, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > 30_000 {
                return CompatibilityAssessment {
                    monitoring_mode: MonitoringMode::Full,
                    anti_cheat_provider: None,
                    compatible_reason: None,
                    unsupported_issue: Some(CompatibilityIssue {
                        kind: "compatibility",
                        rule: "scan-limit",
                        path: install_dir.to_path_buf(),
                    }),
                };
            }
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if anti_cheat.is_none() {
                if let Some(rule) = ANTI_CHEAT_PATTERNS
                    .iter()
                    .find(|rule| name.contains(**rule))
                {
                    anti_cheat = Some((rule, entry_path.clone()));
                }
            }
            if ea_secondary_launcher.is_none() {
                if let Some(rule) = EA_SECONDARY_LAUNCHER_PATTERNS
                    .iter()
                    .find(|rule| name.contains(**rule))
                {
                    ea_secondary_launcher = Some((rule, entry_path.clone()));
                }
            }
            if secondary_launcher.is_none() {
                if let Some(rule) = SECONDARY_LAUNCHER_PATTERNS
                    .iter()
                    .find(|rule| name.contains(**rule))
                {
                    secondary_launcher = Some((rule, entry_path.clone()));
                }
            }
            if entry_path.is_dir() && depth < 8 {
                pending.push((entry_path, depth + 1));
            }
        }
    }
    if let Some((rule, path)) = secondary_launcher {
        return CompatibilityAssessment {
            monitoring_mode: MonitoringMode::Full,
            anti_cheat_provider: None,
            compatible_reason: None,
            unsupported_issue: Some(CompatibilityIssue {
                kind: "secondary-launcher",
                rule,
                path,
            }),
        };
    }
    if let Some((_rule, _path)) = ea_secondary_launcher {
        return CompatibilityAssessment {
            monitoring_mode: MonitoringMode::Compatible,
            anti_cheat_provider: anti_cheat.as_ref().and_then(|(rule, _)| {
                ANTI_CHEAT_PROVIDER_NAMES
                    .iter()
                    .find(|(pattern, _)| *pattern == *rule)
                    .map(|(_, provider)| (*provider).to_string())
            }),
            compatible_reason: Some("secondary-launcher-ea".to_string()),
            unsupported_issue: None,
        };
    }
    if let Some((rule, _path)) = anti_cheat {
        return CompatibilityAssessment {
            monitoring_mode: MonitoringMode::Compatible,
            anti_cheat_provider: Some(
                ANTI_CHEAT_PROVIDER_NAMES
                    .iter()
                    .find(|(pattern, _)| *pattern == rule)
                    .map(|(_, provider)| (*provider).to_string())
                    .unwrap_or_else(|| "unknown-anti-cheat".to_string()),
            ),
            compatible_reason: None,
            unsupported_issue: None,
        };
    }
    CompatibilityAssessment {
        monitoring_mode: MonitoringMode::Full,
        anti_cheat_provider: None,
        compatible_reason: None,
        unsupported_issue: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessInfo {
    pid: u32,
    path: PathBuf,
}

#[derive(Debug, Default)]
struct ProcessObservation {
    confirmed_alive: HashSet<u32>,
    new_candidates: Vec<ProcessInfo>,
    new_confirmed: Vec<ProcessInfo>,
}

impl ProcessObservation {
    fn confirmed_pids(&self) -> HashSet<u32> {
        self.confirmed_alive.clone()
    }
}

#[derive(Debug)]
struct ProcessTracker {
    install_dir: PathBuf,
    monitoring_mode: MonitoringMode,
    known_pids: HashSet<u32>,
    candidates: HashMap<u32, (u8, ProcessInfo)>,
    confirmed: HashMap<u32, ProcessInfo>,
}

impl ProcessTracker {
    fn new_with_mode(
        install_dir: PathBuf,
        initial: Vec<ProcessInfo>,
        monitoring_mode: MonitoringMode,
    ) -> Self {
        Self {
            install_dir,
            monitoring_mode,
            known_pids: initial.into_iter().map(|process| process.pid).collect(),
            candidates: HashMap::new(),
            confirmed: HashMap::new(),
        }
    }

    fn observe(&mut self, processes: &[ProcessInfo]) -> ProcessObservation {
        let current: HashMap<u32, ProcessInfo> = processes
            .iter()
            .filter(|process| {
                is_process_candidate(process, &self.install_dir, self.monitoring_mode)
            })
            .map(|process| (process.pid, process.clone()))
            .collect();
        let current_ids: HashSet<u32> = processes.iter().map(|process| process.pid).collect();
        self.candidates.retain(|pid, _| current_ids.contains(pid));
        self.confirmed.retain(|pid, _| current_ids.contains(pid));

        let mut new_candidates = Vec::new();
        let mut new_confirmed = Vec::new();
        for (pid, process) in current {
            if self.confirmed.contains_key(&pid)
                || (self.known_pids.contains(&pid) && !self.candidates.contains_key(&pid))
            {
                continue;
            }
            let entry = self
                .candidates
                .entry(pid)
                .or_insert_with(|| (0, process.clone()));
            if entry.0 == 0 {
                new_candidates.push(process.clone());
            }
            entry.0 = entry.0.saturating_add(1);
            if entry.0 >= 2 {
                let confirmed = entry.1.clone();
                self.confirmed.insert(pid, confirmed.clone());
                new_confirmed.push(confirmed);
            }
        }
        self.known_pids.extend(current_ids);
        let confirmed_alive = self.confirmed.keys().copied().collect();
        ProcessObservation {
            confirmed_alive,
            new_candidates,
            new_confirmed,
        }
    }
}

fn is_process_candidate(
    process: &ProcessInfo,
    install_dir: &Path,
    monitoring_mode: MonitoringMode,
) -> bool {
    if !path_is_within(&process.path, install_dir) {
        return false;
    }
    let name = process_name(process);
    if monitoring_mode == MonitoringMode::Compatible
        && (is_anti_cheat_process(process)
            || is_ea_launcher_process(process)
            || is_ea_anticheat_process(process)
            || name.starts_with("steam"))
    {
        return false;
    }
    ![
        "crashpad_handler",
        "crashreporter",
        "unitycrashhandler",
        "unins",
        "uninstall",
        "dxsetup",
        "vc_redist",
    ]
    .iter()
    .any(|pattern| name.starts_with(pattern))
}

fn is_anti_cheat_process(process: &ProcessInfo) -> bool {
    let name = process_name(process);
    [
        "easyanticheat",
        "start_protected_game",
        "beservice",
        "beclient",
        "belancher",
        "gameguard",
        "xigncode",
        "pnkbstr",
    ]
    .iter()
    .any(|pattern| name.starts_with(pattern))
}

fn process_name(process: &ProcessInfo) -> String {
    process
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase()
}

fn is_ea_launcher_process(process: &ProcessInfo) -> bool {
    let name = process_name(process);
    [
        "originwebhelperservice",
        "originclientservice",
        "originclient",
        "origin",
        "eabackgroundservice",
        "eadesktop",
        "eaapp",
        "ealauncher",
    ]
    .iter()
    .any(|pattern| name.starts_with(pattern))
}

fn is_ea_anticheat_process(process: &ProcessInfo) -> bool {
    let name = process_name(process);
    name.starts_with("eaanticheat") || name.starts_with("eac") && name.contains("service")
}

fn path_is_within(path: &Path, parent: &Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    path.starts_with(parent)
}

fn snapshot_processes() -> Vec<ProcessInfo> {
    #[cfg(windows)]
    {
        return windows_process_snapshot();
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[derive(Debug, Default)]
struct EdenProcessProbe {
    processes: Vec<ProcessInfo>,
    game_argument_seen: bool,
    game_argument_pids: HashSet<u32>,
}

fn snapshot_processes_for_executable(executable_path: &Path) -> Vec<ProcessInfo> {
    snapshot_processes()
        .into_iter()
        .filter(|process| same_process_path(&process.path, executable_path))
        .collect()
}

fn inspect_eden_game_processes(executable_path: &Path, game_path: &Path) -> EdenProcessProbe {
    let mut probe = EdenProcessProbe {
        processes: snapshot_processes_for_executable(executable_path),
        game_argument_seen: false,
        game_argument_pids: HashSet::new(),
    };
    #[cfg(windows)]
    {
        #[derive(Debug, Deserialize)]
        struct EdenProcessRecord {
            pid: u32,
            #[serde(rename = "executablePath")]
            executable_path: Option<String>,
            #[serde(rename = "gameArgument")]
            game_argument: bool,
        }
        let executable = powershell_literal(&executable_path.to_string_lossy());
        let game = powershell_literal(&game_path.to_string_lossy());
        let script = format!(
            "$e='{executable}';$g='{game}';$rows=Get-CimInstance Win32_Process | Where-Object {{ $_.ProcessId -ne $PID -and (( $_.ExecutablePath -and $_.ExecutablePath -ieq $e) -or ($_.CommandLine -and $_.CommandLine -imatch [regex]::Escape($g))) }} | ForEach-Object {{ [pscustomobject]@{{ pid=[int]$_.ProcessId; executablePath=$_.ExecutablePath; gameArgument=[bool]($_.CommandLine -and $_.CommandLine -imatch [regex]::Escape($g)) }} }}; if($null -eq $rows) {{ '[]' }} else {{ @($rows) | ConvertTo-Json -Compress }}",
        );
        use std::os::windows::process::CommandExt;
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .output();
        if let Ok(output) = output {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                let records = if value.is_array() {
                    serde_json::from_value::<Vec<EdenProcessRecord>>(value).unwrap_or_default()
                } else {
                    serde_json::from_value::<EdenProcessRecord>(value)
                        .map(|record| vec![record])
                        .unwrap_or_default()
                };
                for record in records {
                    probe.game_argument_seen |= record.game_argument;
                    if record.game_argument {
                        probe.game_argument_pids.insert(record.pid);
                    }
                    if let Some(path) = record.executable_path {
                        if !probe
                            .processes
                            .iter()
                            .any(|process| process.pid == record.pid)
                        {
                            probe.processes.push(ProcessInfo {
                                pid: record.pid,
                                path: PathBuf::from(path),
                            });
                        }
                    }
                }
            }
        }
    }
    probe
}

fn same_process_path(left: &Path, right: &Path) -> bool {
    normalize_process_path(left) == normalize_process_path(right)
}

fn normalize_process_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

#[cfg(windows)]
fn powershell_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn eden_telemetry_details(
    game_id: &str,
    title_id: Option<&str>,
    event: &str,
    result: &str,
) -> String {
    format!(
        "platform=nintendo_switch emulator=eden game_id={} titleId={} event={} launchResult={}",
        sanitize_id(game_id),
        title_id
            .map(sanitize_id)
            .unwrap_or_else(|| "unknown".to_string()),
        event,
        result.replace([' ', '\n', '\r'], "_")
    )
}

fn eden_game_is_active(
    state: GameSessionState,
    game_argument_seen: bool,
    new_process_detected: bool,
    spawned_process_exited: bool,
) -> bool {
    match state {
        GameSessionState::Launching => game_argument_seen || new_process_detected,
        GameSessionState::Running | GameSessionState::Finishing => {
            if spawned_process_exited {
                new_process_detected
            } else {
                game_argument_seen || new_process_detected
            }
        }
        _ => false,
    }
}

#[cfg(windows)]
fn windows_process_snapshot() -> Vec<ProcessInfo> {
    use std::mem;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut result = Vec::new();
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if let Some(path) = process_path(entry.th32ProcessID) {
                    result.push(ProcessInfo {
                        pid: entry.th32ProcessID,
                        path,
                    });
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        result
    }
}

#[cfg(windows)]
unsafe fn process_path(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if handle.is_null() {
        return None;
    }
    let mut buffer = [0u16; 32_768];
    let mut length = buffer.len() as u32;
    let result =
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut length);
    CloseHandle(handle);
    if result == 0 || length == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

fn launch_steam_app(app_id: i64) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::ptr;
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let uri = to_wide(&format!("steam://rungameid/{app_id}"));
        let operation = to_wide("open");
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                uri.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if (result as isize) <= 32 {
            return Err(format!("ShellExecuteW returned {}", result as isize));
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = app_id;
        Err("Steam game sessions require Windows".to_string())
    }
}

#[cfg(windows)]
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn resolve_rtx_hdr_executable(
    database: &settings::DatabaseState,
    game_id: &str,
    install_dir: &Path,
) -> Result<Option<PathBuf>, String> {
    let profile = settings::get_display_profile(database, game_id)
        .map_err(|error| format!("display-profile-read:{error}"))?;
    if profile.rtx_hdr_preset.is_none() && profile.hdr_mode != crate::display::DisplayHdrMode::Auto
    {
        return Ok(None);
    }
    if let Ok(frame_profile) = settings::get_frame_generation_profile(database, game_id) {
        if let Some(target) = frame_profile.target_executable {
            let path = PathBuf::from(target);
            if path.is_file() {
                return Ok(Some(path));
            }
        }
    }
    Ok(find_game_executable(install_dir))
}

fn find_game_executable(install_dir: &Path) -> Option<PathBuf> {
    let mut pending = vec![(install_dir.to_path_buf(), 0_u8)];
    let mut candidates = Vec::new();
    while let Some((directory, depth)) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && depth < 5 {
                pending.push((path, depth + 1));
                continue;
            }
            if !path.is_file()
                || !path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if ["launcher", "crash", "unins", "setup", "install", "redist"]
                .iter()
                .any(|token| name.contains(token))
            {
                continue;
            }
            let normalized = path
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            if normalized.contains("/__overlay/") {
                continue;
            }
            let directory_score = if normalized.contains("/binaries/win64/") {
                0_u8
            } else if normalized.contains("/binaries/") {
                1_u8
            } else {
                2_u8
            };
            let shipping_score = u8::from(name.contains("shipping"));
            candidates.push((directory_score + shipping_score, name.len(), path));
        }
    }
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    candidates.into_iter().next().map(|candidate| candidate.2)
}

fn display_profile_failure_message(error: &str) -> String {
    if error.contains("RTX_HDR") || error.contains("NVIDIA_APP_OVERLAY") {
        "RTX HDR no pudo aplicarse; verifica NVIDIA App, Overlay y Game Filters. El juego no se iniciará."
            .to_string()
    } else {
        "No se pudo aplicar el perfil de pantalla; el juego no se iniciará.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        duration_since_timestamp, eden_game_is_active, eden_telemetry_details,
        find_game_executable, inspect_compatibility, is_process_candidate, path_is_within,
        GameSessionState, MonitoringMode, ProcessInfo, ProcessTracker,
    };

    #[test]
    fn eden_telemetry_details_do_not_include_sensitive_paths() {
        let details = eden_telemetry_details(
            "game-D:\\Nintendo\\ROMs\\Mario Kart 8 Deluxe.xci",
            Some("0100ABCDEF012345"),
            "launch_command_prepared",
            "success",
        );
        assert!(details.contains("platform=nintendo_switch"));
        assert!(details.contains("emulator=eden"));
        assert!(details.contains("titleId=0100ABCDEF012345"));
        assert!(!details.contains("D:\\Nintendo\\ROMs"));
        assert!(!details.contains("Mario Kart 8 Deluxe"));
    }

    #[test]
    fn eden_process_alive_alone_is_not_a_running_game() {
        assert!(eden_game_is_active(
            GameSessionState::Launching,
            false,
            true,
            false
        ));
        assert!(eden_game_is_active(
            GameSessionState::Running,
            true,
            false,
            false
        ));
        assert!(!eden_game_is_active(
            GameSessionState::Running,
            true,
            false,
            true
        ));
        assert!(eden_game_is_active(
            GameSessionState::Running,
            false,
            true,
            true
        ));
        assert!(eden_game_is_active(
            GameSessionState::Finishing,
            true,
            false,
            false
        ));
    }
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn candidate_requires_two_consecutive_observations() {
        let root = PathBuf::from(r"C:\Games\Example");
        let process = ProcessInfo {
            pid: 42,
            path: root.join("game.exe"),
        };
        let mut tracker =
            ProcessTracker::new_with_mode(root.clone(), Vec::new(), MonitoringMode::Full);
        assert!(tracker
            .observe(std::slice::from_ref(&process))
            .confirmed_alive
            .is_empty());
        assert_eq!(
            tracker.observe(&[process]).confirmed_alive,
            [42].into_iter().collect()
        );
    }

    #[test]
    fn process_outside_install_dir_is_rejected() {
        let process = ProcessInfo {
            pid: 42,
            path: PathBuf::from(r"C:\Other\game.exe"),
        };
        assert!(!is_process_candidate(
            &process,
            PathBuf::from(r"C:\Games").as_path(),
            MonitoringMode::Full,
        ));
    }

    #[test]
    fn process_exclusions_are_narrow() {
        let root = PathBuf::from(r"C:\Games\Example");
        assert!(!is_process_candidate(
            &ProcessInfo {
                pid: 1,
                path: root.join("crashpad_handler.exe")
            },
            &root,
            MonitoringMode::Full,
        ));
        assert!(is_process_candidate(
            &ProcessInfo {
                pid: 2,
                path: root.join("launcher-game.exe")
            },
            &root,
            MonitoringMode::Full,
        ));
    }

    #[test]
    fn anti_cheat_selects_compatible_monitoring() {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-session-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("EasyAntiCheat")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Compatible);
        assert_eq!(
            assessment.anti_cheat_provider.as_deref(),
            Some("easy-anticheat")
        );
        assert!(assessment.unsupported_issue.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn origin_selects_ea_compatible_monitoring() {
        let root = temporary_install_dir("origin");
        fs::create_dir_all(root.join("Origin")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Compatible);
        assert_eq!(
            assessment.compatible_reason.as_deref(),
            Some("secondary-launcher-ea")
        );
        assert!(assessment.unsupported_issue.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ea_app_selects_compatible_monitoring_and_preserves_anticheat_context() {
        let root = temporary_install_dir("ea-app");
        fs::create_dir_all(root.join("EADesktop")).unwrap();
        fs::create_dir_all(root.join("EAAntiCheat.GameService")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Compatible);
        assert_eq!(
            assessment.compatible_reason.as_deref(),
            Some("secondary-launcher-ea")
        );
        assert_eq!(
            assessment.anti_cheat_provider.as_deref(),
            Some("ea-anticheat")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ubisoft_remains_unsupported() {
        let root = temporary_install_dir("ubisoft");
        fs::create_dir_all(root.join("UbisoftConnect")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Full);
        assert_eq!(
            assessment
                .unsupported_issue
                .as_ref()
                .map(|issue| issue.kind),
            Some("secondary-launcher")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_steam_selects_full_monitoring() {
        let root = temporary_install_dir("native");
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Full);
        assert!(assessment.anti_cheat_provider.is_none());
        assert!(assessment.unsupported_issue.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn battleye_selects_compatible_monitoring() {
        let root = temporary_install_dir("battleye");
        fs::create_dir_all(root.join("BattlEye")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Compatible);
        assert_eq!(assessment.anti_cheat_provider.as_deref(), Some("battleye"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ea_anticheat_selects_compatible_monitoring_without_ea_app() {
        let root = temporary_install_dir("ea-anticheat");
        fs::create_dir_all(root.join("EAAntiCheat.GameService")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Compatible);
        assert_eq!(
            assessment.anti_cheat_provider.as_deref(),
            Some("ea-anticheat")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn secondary_launcher_remains_unsupported_even_with_anti_cheat() {
        let root = temporary_install_dir("secondary-launcher");
        fs::create_dir_all(root.join("EasyAntiCheat")).unwrap();
        fs::create_dir_all(root.join("Epic Games Launcher")).unwrap();
        let assessment = inspect_compatibility(&root);
        assert_eq!(assessment.monitoring_mode, MonitoringMode::Full);
        assert_eq!(
            assessment
                .unsupported_issue
                .as_ref()
                .map(|issue| issue.kind),
            Some("secondary-launcher")
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn temporary_install_dir(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-session-test-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn compatible_monitoring_does_not_confirm_the_anti_cheat_process() {
        let root = PathBuf::from(r"C:\Games\Example");
        assert!(!is_process_candidate(
            &ProcessInfo {
                pid: 42,
                path: root.join("EasyAntiCheat_EOS.exe"),
            },
            &root,
            MonitoringMode::Compatible,
        ));
        assert!(!is_process_candidate(
            &ProcessInfo {
                pid: 44,
                path: root.join("Steam.exe"),
            },
            &root,
            MonitoringMode::Compatible,
        ));
        for (pid, executable) in [
            (46, "EADesktop.exe"),
            (47, "OriginWebHelperService.exe"),
            (48, "EABackgroundService.exe"),
            (49, "EAAntiCheat.GameService.exe"),
        ] {
            assert!(!is_process_candidate(
                &ProcessInfo {
                    pid,
                    path: root.join(executable),
                },
                &root,
                MonitoringMode::Compatible,
            ));
        }
        assert!(is_process_candidate(
            &ProcessInfo {
                pid: 43,
                path: root.join("MarvelTokon.exe"),
            },
            &root,
            MonitoringMode::Compatible,
        ));
    }

    #[test]
    fn compatible_monitoring_confirms_a_visible_game_process_after_two_observations() {
        let root = PathBuf::from(r"C:\Games\Example");
        let process = ProcessInfo {
            pid: 45,
            path: root.join("MarvelTokon.exe"),
        };
        let mut tracker =
            ProcessTracker::new_with_mode(root, Vec::new(), MonitoringMode::Compatible);
        assert!(tracker
            .observe(std::slice::from_ref(&process))
            .confirmed_alive
            .is_empty());
        assert_eq!(
            tracker.observe(&[process]).confirmed_alive,
            [45].into_iter().collect()
        );
    }

    #[test]
    fn duration_is_non_negative_and_uses_seconds() {
        assert_eq!(duration_since_timestamp("0"), super::unix_seconds() as i64);
        assert_eq!(duration_since_timestamp("not-a-timestamp"), 0);
    }

    #[test]
    fn paths_are_checked_against_install_dir() {
        assert!(path_is_within(
            PathBuf::from(r"C:\Games\Example\game.exe").as_path(),
            PathBuf::from(r"C:\Games\Example").as_path(),
        ));
        assert!(!path_is_within(
            PathBuf::from(r"C:\Games\Other\game.exe").as_path(),
            PathBuf::from(r"C:\Games\Example").as_path(),
        ));
    }

    #[test]
    fn executable_discovery_prefers_game_binary_over_shipping_variant_and_overlay() {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-rtx-executable-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let binaries = root.join("SwGame").join("Binaries").join("Win64");
        let overlay = root.join("__overlay");
        fs::create_dir_all(&binaries).unwrap();
        fs::create_dir_all(&overlay).unwrap();
        fs::write(binaries.join("starwarsjedifallenorder.exe"), []).unwrap();
        fs::write(binaries.join("SwGame-Win64-Shipping.exe"), []).unwrap();
        fs::write(overlay.join("overlayinjector.exe"), []).unwrap();
        assert_eq!(
            find_game_executable(&root)
                .and_then(|path| path.file_name().map(|name| name.to_owned())),
            Some(std::ffi::OsString::from("starwarsjedifallenorder.exe"))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
