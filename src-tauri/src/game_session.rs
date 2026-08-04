use crate::frame_generation::FrameGenerationProvider;
use crate::lossless_scaling::{is_lossless_scaling_running, LosslessScalingProvider};
use crate::{display, settings, steam};
use serde::Serialize;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSessionStatus {
    pub session_id: String,
    pub game_id: String,
    pub steam_app_id: i64,
    pub state: GameSessionState,
    pub occurred_at: String,
    pub elapsed_seconds: i64,
    pub message: String,
    pub unsupported_reason: Option<String>,
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
    state: GameSessionState,
    install_dir: Option<PathBuf>,
    started_at: Option<String>,
    activity_session_id: Option<i64>,
    tracked_processes: HashSet<u32>,
    display_restore: Option<display::PendingDisplayRestore>,
    display_restore_on_exit: bool,
    message: String,
    unsupported_reason: Option<String>,
}

impl SteamGameSessionService {
    pub fn start(
        &self,
        app: AppHandle,
        game_id: String,
        steam_app_id: i64,
    ) -> Result<GameSessionStatus, SessionCommandError> {
        let session_id = format!("steam-{}-{}", sanitize_id(&game_id), unix_seconds());
        let active = ActiveSession {
            session_id: session_id.clone(),
            game_id: game_id.clone(),
            steam_app_id,
            state: GameSessionState::Preparing,
            install_dir: None,
            started_at: None,
            activity_session_id: None,
            tracked_processes: HashSet::new(),
            display_restore: None,
            display_restore_on_exit: true,
            message: "Comprobando instalación y compatibilidad…".to_string(),
            unsupported_reason: None,
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
        let service = self.clone();
        thread::spawn(move || service.run(app, session_id, game_id, steam_app_id));
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

    fn run(&self, app: AppHandle, session_id: String, game_id: String, requested_app_id: i64) {
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
        if let Some(issue) = inspect_compatibility(&installation.install_dir) {
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
                "Este juego utiliza un launcher o sistema anti-cheat que todavía no está soportado."
                    .to_string(),
                Some(format!("{}:{}", issue.kind, issue.rule)),
            );
            return;
        }
        database.log(
            "game-session",
            "COMPATIBILITY_ACCEPTED",
            &format!(
                "game_id={game_id} install_dir={}",
                installation.install_dir.display()
            ),
        );
        self.update_install_dir(&session_id, installation.install_dir.clone());
        if let Ok(Some(pending)) = settings::get_pending_display_restore(&database) {
            if let Err(error) = display::restore_mode(&pending) {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "Hay una restauracion de pantalla pendiente que no se pudo completar."
                        .to_string(),
                    Some(format!("display-pending-restore:{error}")),
                );
                return;
            }
            if let Err(error) = settings::clear_pending_display_restore(&database) {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "Se restauro la pantalla, pero no se pudo limpiar el estado pendiente."
                        .to_string(),
                    Some(format!("display-pending-clear:{error}")),
                );
                return;
            }
        }

        let profile = match settings::get_display_profile(&database, &game_id) {
            Ok(profile) => profile,
            Err(error) => {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "No se pudo leer el perfil de pantalla del juego.".to_string(),
                    Some(format!("display-profile-read:{error}")),
                );
                return;
            }
        };
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
        if profile.enabled {
            let (Some(width), Some(height), Some(refresh_rate)) =
                (profile.width, profile.height, profile.refresh_rate)
            else {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "El perfil de pantalla esta incompleto y no se aplicara.".to_string(),
                    Some("display-profile-incomplete".to_string()),
                );
                return;
            };
            let current = match display::current_mode(profile.display_id.as_deref()) {
                Ok(mode) => mode,
                Err(error) => {
                    self.fail(
                        &app,
                        &session_id,
                        GameSessionState::Error,
                        "No se pudo obtener el modo actual de la pantalla.".to_string(),
                        Some(format!("display-current:{error}")),
                    );
                    return;
                }
            };
            let pending = display::PendingDisplayRestore {
                display_id: current.display_id.clone(),
                width: current.width,
                height: current.height,
                refresh_rate: current.refresh_rate,
                created_at: timestamp_now(),
            };
            if let Err(error) = settings::save_pending_display_restore(&database, &pending) {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "No se pudo guardar la restauracion segura de la pantalla.".to_string(),
                    Some(format!("display-pending-save:{error}")),
                );
                return;
            }
            self.set_display_restore(&session_id, pending, profile.restore_on_exit);
            self.set_state(
                &app,
                &session_id,
                GameSessionState::Preparing,
                "Preparando pantalla...".to_string(),
                None,
            );
            let requested = display::DisplayMode {
                display_id: profile.display_id.unwrap_or(current.display_id),
                device_name: profile.device_name.unwrap_or_default(),
                width,
                height,
                refresh_rate,
            };
            if let Err(error) = display::apply_mode(&requested) {
                self.fail(
                    &app,
                    &session_id,
                    GameSessionState::Error,
                    "No se pudo aplicar el perfil de pantalla; el juego no se iniciara."
                        .to_string(),
                    Some(format!("display-apply:{error}")),
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

        let mut tracker =
            ProcessTracker::new(installation.install_dir.clone(), snapshot_processes());
        let mut frame_generation_target_learned = frame_generation_profile
            .target_executable
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty());
        let launch_deadline = Instant::now() + Duration::from_secs(120);
        let mut state = GameSessionState::Launching;
        let mut finishing_since: Option<Instant> = None;

        loop {
            let observation = tracker.observe(&snapshot_processes());
            self.update_tracked_processes(&session_id, observation.confirmed_pids());
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
                        self.emit_current(&app);
                        state = GameSessionState::Running;
                    } else if Instant::now() >= launch_deadline {
                        database.log(
                            "game-session",
                            "TIMEOUT",
                            &format!("game_id={game_id} timeout_seconds=120"),
                        );
                        self.fail(
                            &app,
                            &session_id,
                            GameSessionState::Error,
                            "Steam recibió la solicitud, pero LumaDeck no pudo confirmar el inicio del juego."
                                .to_string(),
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
                        if let Err(error) = self.finish_activity(&database, &session_id, &game_id) {
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
                        self.finish(&app, &session_id);
                        return;
                    }
                }
                _ => return,
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

    fn update_tracked_processes(&self, session_id: &str, pids: HashSet<u32>) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.tracked_processes = pids;
            }
        }
    }

    fn set_display_restore(
        &self,
        session_id: &str,
        pending: display::PendingDisplayRestore,
        restore_on_exit: bool,
    ) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.display_restore = Some(pending);
                session.display_restore_on_exit = restore_on_exit;
            }
        }
    }

    fn restore_display(
        &self,
        app: &AppHandle,
        session_id: &str,
        force: bool,
    ) -> Result<(), String> {
        let (pending, restore_on_exit) = self
            .active
            .lock()
            .map_err(|_| "display-session-lock".to_string())?
            .as_ref()
            .filter(|session| session.session_id == session_id)
            .map(|session| {
                (
                    session.display_restore.clone(),
                    session.display_restore_on_exit,
                )
            })
            .unwrap_or((None, true));
        let Some(pending) = pending else {
            return Ok(());
        };
        let database = app.state::<settings::DatabaseState>();
        if force || restore_on_exit {
            display::restore_mode(&pending)?;
        }
        settings::clear_pending_display_restore(&database).map_err(|error| error.to_string())?;
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.display_restore = None;
            }
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

    fn finish_activity(
        &self,
        database: &settings::DatabaseState,
        session_id: &str,
        game_id: &str,
    ) -> Result<(), String> {
        let activity_session_id = self
            .active
            .lock()
            .ok()
            .and_then(|current| {
                current
                    .as_ref()
                    .filter(|session| session.session_id == session_id)
                    .and_then(|session| session.activity_session_id)
            })
            .ok_or_else(|| "missing-activity-session-id".to_string())?;
        settings::end_game_session(database, game_id, activity_session_id, false)
            .map_err(|error| error.to_string())
    }

    fn set_state(
        &self,
        app: &AppHandle,
        session_id: &str,
        state: GameSessionState,
        message: String,
        unsupported_reason: Option<String>,
    ) {
        if let Ok(mut current) = self.active.lock() {
            if let Some(session) = current
                .as_mut()
                .filter(|session| session.session_id == session_id)
            {
                session.state = state;
                session.message = message;
                session.unsupported_reason = unsupported_reason;
            }
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

fn status_from_session(session: &ActiveSession) -> GameSessionStatus {
    GameSessionStatus {
        session_id: session.session_id.clone(),
        game_id: session.game_id.clone(),
        steam_app_id: session.steam_app_id,
        state: session.state,
        occurred_at: timestamp_now(),
        elapsed_seconds: session
            .started_at
            .as_deref()
            .map(duration_since_timestamp)
            .unwrap_or_default(),
        message: session.message.clone(),
        unsupported_reason: session.unsupported_reason.clone(),
    }
}

fn idle_status() -> GameSessionStatus {
    GameSessionStatus {
        session_id: String::new(),
        game_id: String::new(),
        steam_app_id: 0,
        state: GameSessionState::Idle,
        occurred_at: timestamp_now(),
        elapsed_seconds: 0,
        message: String::new(),
        unsupported_reason: None,
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

const ANTI_CHEAT_PATTERNS: &[&str] = &[
    "easyanticheat",
    "easyanticheat_eos",
    "start_protected_game",
    "battleye",
    "beservice",
    "beclient",
    "eaanticheat",
    "eaanticheat.gameservice",
    "pnkbstr",
    "gameguard",
    "xigncode",
];

const SECONDARY_LAUNCHER_PATTERNS: &[&str] = &[
    "ubisoftconnect",
    "ubisoftgamelauncher",
    "uplay",
    "eadesktop",
    "ealauncher",
    "origin",
    "rockstar games launcher",
    "socialclub",
    "battle.net",
];

fn inspect_compatibility(install_dir: &Path) -> Option<CompatibilityIssue> {
    let mut pending = vec![(install_dir.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    while let Some((path, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > 30_000 {
                return Some(CompatibilityIssue {
                    kind: "compatibility",
                    rule: "scan-limit",
                    path: install_dir.to_path_buf(),
                });
            }
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if let Some(rule) = ANTI_CHEAT_PATTERNS
                .iter()
                .find(|rule| name.contains(**rule))
            {
                return Some(CompatibilityIssue {
                    kind: "anti-cheat",
                    rule,
                    path: entry_path,
                });
            }
            if let Some(rule) = SECONDARY_LAUNCHER_PATTERNS
                .iter()
                .find(|rule| name.contains(**rule))
            {
                return Some(CompatibilityIssue {
                    kind: "secondary-launcher",
                    rule,
                    path: entry_path,
                });
            }
            if entry_path.is_dir() && depth < 8 {
                pending.push((entry_path, depth + 1));
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessInfo {
    pid: u32,
    path: PathBuf,
}

#[derive(Debug, Default)]
struct ProcessObservation {
    confirmed_alive: HashSet<u32>,
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
    known_pids: HashSet<u32>,
    candidates: HashMap<u32, (u8, ProcessInfo)>,
    confirmed: HashMap<u32, ProcessInfo>,
}

impl ProcessTracker {
    fn new(install_dir: PathBuf, initial: Vec<ProcessInfo>) -> Self {
        Self {
            install_dir,
            known_pids: initial.into_iter().map(|process| process.pid).collect(),
            candidates: HashMap::new(),
            confirmed: HashMap::new(),
        }
    }

    fn observe(&mut self, processes: &[ProcessInfo]) -> ProcessObservation {
        let current: HashMap<u32, ProcessInfo> = processes
            .iter()
            .filter(|process| is_process_candidate(process, &self.install_dir))
            .map(|process| (process.pid, process.clone()))
            .collect();
        let current_ids: HashSet<u32> = processes.iter().map(|process| process.pid).collect();
        self.candidates.retain(|pid, _| current_ids.contains(pid));
        self.confirmed.retain(|pid, _| current_ids.contains(pid));

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
            new_confirmed,
        }
    }
}

fn is_process_candidate(process: &ProcessInfo, install_dir: &Path) -> bool {
    if !path_is_within(&process.path, install_dir) {
        return false;
    }
    let name = process
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
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

#[cfg(test)]
mod tests {
    use super::{
        duration_since_timestamp, inspect_compatibility, is_process_candidate, path_is_within,
        ProcessInfo, ProcessTracker,
    };
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
        let mut tracker = ProcessTracker::new(root.clone(), Vec::new());
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
            PathBuf::from(r"C:\Games").as_path()
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
        ));
        assert!(is_process_candidate(
            &ProcessInfo {
                pid: 2,
                path: root.join("launcher-game.exe")
            },
            &root,
        ));
    }

    #[test]
    fn compatibility_blocks_known_markers() {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-session-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("EasyAntiCheat")).unwrap();
        let issue = inspect_compatibility(&root).expect("anti-cheat marker");
        assert_eq!(issue.kind, "anti-cheat");
        fs::remove_dir_all(root).unwrap();
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
}
