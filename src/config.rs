use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DisplayConfig {
    pub name: String,
    pub geometry: String,
    pub video_path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub mute: bool,
    #[serde(default = "default_volume")]
    pub volume: u32,
}

fn default_true() -> bool {
    true
}

fn default_volume() -> u32 {
    100
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Copy)]
pub enum DesktopLayerMode {
    Standard,         // Dưới biểu tượng Desktop (-b -un -s -st -sp)
    OverrideRedirect, // Override Redirect mode (-ov -b -un -s -st -sp)
    ForceDesktopType, // Desktop Type window (-fdt -b)
}

impl Default for DesktopLayerMode {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GlobalSettings {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub pause_on_fullscreen: bool,
    #[serde(default)]
    pub layer_mode: DesktopLayerMode,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            autostart: false,
            pause_on_fullscreen: true,
            layer_mode: DesktopLayerMode::Standard,
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub displays: Vec<DisplayConfig>,
    #[serde(default)]
    pub settings: GlobalSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            displays: vec![
                DisplayConfig {
                    name: "HDMI-1".to_string(),
                    geometry: "1920x1080+0+0".to_string(),
                    video_path: "".to_string(),
                    enabled: true,
                    mute: true,
                    volume: 100,
                },
            ],
            settings: GlobalSettings::default(),
        }
    }
}

impl AppConfig {
    pub fn get_config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("livewallpaper")
    }

    pub fn get_config_path() -> PathBuf {
        Self::get_config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::get_config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }

        // If file doesn't exist or is corrupted, create default config and save it
        let default_cfg = AppConfig::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::get_config_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .with_context(|| format!("Không thể tạo thư mục cấu hình tại {:?}", dir))?;
        }

        let path = Self::get_config_path();
        let content = serde_json::to_string_pretty(self)
            .context("Không thể chuyển đổi cấu hình sang định dạng JSON")?;

        fs::write(&path, content)
            .with_context(|| format!("Không thể lưu file cấu hình tại {:?}", path))?;

        Ok(())
    }

    pub fn get_display_mut(&mut self, name: &str) -> Option<&mut DisplayConfig> {
        self.displays.iter_mut().find(|d| d.name == name)
    }

    pub fn update_or_add_display(&mut self, new_disp: DisplayConfig) {
        if let Some(existing) = self.get_display_mut(&new_disp.name) {
            existing.geometry = new_disp.geometry;
            if existing.video_path.is_empty() && !new_disp.video_path.is_empty() {
                existing.video_path = new_disp.video_path;
            }
        } else {
            self.displays.push(new_disp);
        }
    }
}
