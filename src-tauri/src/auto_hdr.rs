use serde::{Deserialize, Serialize};
use std::path::Path;

const USER_GPU_PREFERENCES_KEY: &str = r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoHdrSnapshot {
    pub executable: String,
    pub existed: bool,
    pub value: Option<String>,
}

pub fn capture(executable: &Path) -> Result<AutoHdrSnapshot, String> {
    #[cfg(windows)]
    {
        let executable = executable_string(executable)?;
        let value = query_value(&executable)?;
        return Ok(AutoHdrSnapshot {
            executable,
            existed: value.is_some(),
            value,
        });
    }
    #[cfg(not(windows))]
    {
        Ok(AutoHdrSnapshot {
            executable: executable.display().to_string(),
            existed: false,
            value: None,
        })
    }
}

pub fn disable(executable: &Path) -> Result<AutoHdrSnapshot, String> {
    let snapshot = capture(executable)?;
    #[cfg(windows)]
    {
        let current = snapshot.value.as_deref().unwrap_or_default();
        let next = with_auto_hdr_flag(current, false);
        set_value(&snapshot.executable, &next)?;
        if query_value(&snapshot.executable)?.as_deref() != Some(next.as_str()) {
            return Err("AUTO_HDR_VERIFY_FAILED".to_string());
        }
    }
    Ok(snapshot)
}

pub fn restore(snapshot: &AutoHdrSnapshot) -> Result<(), String> {
    #[cfg(windows)]
    {
        if snapshot.existed {
            set_value(
                &snapshot.executable,
                snapshot.value.as_deref().unwrap_or_default(),
            )?;
        } else {
            delete_value(&snapshot.executable)?;
        }
        let restored = query_value(&snapshot.executable)?;
        if restored != snapshot.value {
            return Err("AUTO_HDR_RESTORE_VERIFY_FAILED".to_string());
        }
    }
    Ok(())
}

fn with_auto_hdr_flag(value: &str, enabled: bool) -> String {
    let mut entries = value
        .split(';')
        .filter(|entry| {
            let normalized = entry.trim().to_ascii_lowercase();
            !normalized.starts_with("autohdr")
        })
        .filter(|entry| !entry.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    entries.push(format!("AutoHDREnable={}", i32::from(enabled)));
    format!("{};", entries.join(";"))
}

#[cfg(windows)]
fn executable_string(executable: &Path) -> Result<String, String> {
    std::fs::canonicalize(executable)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| format!("AUTO_HDR_EXECUTABLE_INVALID:{error}"))
}

#[cfg(windows)]
fn query_value(executable: &str) -> Result<Option<String>, String> {
    let output = std::process::Command::new("reg.exe")
        .args(["query", USER_GPU_PREFERENCES_KEY, "/v", executable])
        .output()
        .map_err(|error| format!("AUTO_HDR_REGISTRY_READ_FAILED:{error}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().find_map(|line| {
        let marker = "REG_SZ";
        let index = line.find(marker)?;
        Some(line[index + marker.len()..].trim().to_string())
    }))
}

#[cfg(windows)]
fn set_value(executable: &str, value: &str) -> Result<(), String> {
    let output = std::process::Command::new("reg.exe")
        .args([
            "add",
            USER_GPU_PREFERENCES_KEY,
            "/v",
            executable,
            "/t",
            "REG_SZ",
            "/d",
            value,
            "/f",
        ])
        .output()
        .map_err(|error| format!("AUTO_HDR_REGISTRY_WRITE_FAILED:{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "AUTO_HDR_REGISTRY_WRITE_FAILED:{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(windows)]
fn delete_value(executable: &str) -> Result<(), String> {
    let output = std::process::Command::new("reg.exe")
        .args(["delete", USER_GPU_PREFERENCES_KEY, "/v", executable, "/f"])
        .output()
        .map_err(|error| format!("AUTO_HDR_REGISTRY_DELETE_FAILED:{error}"))?;
    if output.status.success() || query_value(executable)?.is_none() {
        Ok(())
    } else {
        Err(format!(
            "AUTO_HDR_REGISTRY_DELETE_FAILED:{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{with_auto_hdr_flag, AutoHdrSnapshot};

    #[test]
    fn auto_hdr_flag_is_replaced_without_touching_other_preferences() {
        assert_eq!(
            with_auto_hdr_flag("GpuPreference=2;AutoHDREnable=1;", false),
            "GpuPreference=2;AutoHDREnable=0;"
        );
    }

    #[test]
    fn missing_registry_value_is_represented_as_inherited() {
        let snapshot = AutoHdrSnapshot {
            executable: "game.exe".to_string(),
            existed: false,
            value: None,
        };
        assert!(!snapshot.existed);
        assert!(snapshot.value.is_none());
    }
}
