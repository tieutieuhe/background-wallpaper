use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn get_autostart_filepath() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autostart")
        .join("livewallpaper.desktop")
}

pub fn is_autostart_enabled() -> bool {
    get_autostart_filepath().exists()
}

pub fn set_autostart(enable: bool) -> Result<()> {
    let file_path = get_autostart_filepath();

    if enable {
        let current_exe = env::current_exe()
            .context("Không thể xác định đường dẫn file thực thi ứng dụng")?;
        let exe_str = current_exe.to_string_lossy();

        let desktop_entry = format!(
            "[Desktop Entry]\n\
            Type=Application\n\
            Name=Live Wallpaper Engine\n\
            Comment=Tự động khởi chạy hình nền động khi đăng nhập\n\
            Exec={} --daemon\n\
            Terminal=false\n\
            Categories=Utility;Background;\n\
            X-GNOME-Autostart-enabled=true\n",
            exe_str
        );

        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(&file_path, desktop_entry)
            .with_context(|| format!("Không thể tạo file autostart tại {:?}", file_path))?;
    } else if file_path.exists() {
        fs::remove_file(&file_path)
            .with_context(|| format!("Không thể xóa file autostart tại {:?}", file_path))?;
    }

    Ok(())
}
