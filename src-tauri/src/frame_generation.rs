use crate::settings::FrameGenerationProfile;

pub trait FrameGenerationProvider {
    fn synchronize_if_needed(
        &self,
        profile: &FrameGenerationProfile,
    ) -> Result<FrameGenerationSync, String>;

    fn status(&self) -> LosslessScalingStatus;

    fn start_background(&self) -> Result<(), String>;

    fn ensure_running(&self) -> Result<(), String> {
        self.start_background()
    }

    fn open_application(&self) -> Result<(), String>;

    fn restart_background(&self) -> Result<(), String>;

    fn restore_backup(&self) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct FrameGenerationSync {
    pub restart_required: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LosslessScalingStatus {
    pub status: String,
    pub version: String,
    pub installation_path: Option<String>,
    pub settings_path: String,
    pub settings_status: String,
    pub application_running: bool,
    pub restart_required: bool,
}
