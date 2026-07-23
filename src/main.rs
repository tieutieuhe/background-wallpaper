mod autostart;
mod config;
mod engine;
mod monitor;
mod ui;

use clap::Parser;
use config::AppConfig;
use engine::WallpaperEngine;
use gtk::prelude::*;
use gtk::Application;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{thread, time::Duration};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "LiveWallpaper Engine",
    version = "0.2.0",
    about = "Ứng dụng cài đặt và quản lý Live Wallpaper chuyên nghiệp trên Linux"
)]
struct Args {
    /// Chạy ứng dụng dưới dạng daemon ngầm (không hiển thị cửa sổ GUI)
    #[arg(short, long)]
    daemon: bool,

    /// Dừng ứng dụng đang chạy ngầm và dọn dẹp tất cả hình nền
    #[arg(short, long)]
    stop: bool,
}

fn get_pid_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("livewallpaper")
        .join("livewallpaper.pid")
}

fn is_process_running(pid: u32) -> bool {
    let comm_path = format!("/proc/{}/comm", pid);
    if let Ok(comm) = std::fs::read_to_string(&comm_path) {
        let comm = comm.trim();
        let current_exe_name = std::env::current_exe()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|_| "LiveWallpaper".to_string());

        comm == current_exe_name || comm.to_lowercase().contains("livewallpaper")
    } else {
        false
    }
}

fn kill_process_gracefully(pid: u32) -> bool {
    println!("[LiveWallpaper] Đang gửi tín hiệu dừng tới tiến trình cũ (PID: {})...", pid);
    let _ = std::process::Command::new("kill")
        .args(["-15", &pid.to_string()])
        .status();

    // Chờ tối đa 3 giây để tiến trình cũ kết thúc và dọn dẹp các wallpaper con
    for _ in 0..30 {
        if !is_process_running(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }

    // Nếu vẫn chưa thoát, gửi SIGKILL (kill -9)
    if is_process_running(pid) {
        println!("[LiveWallpaper] Tiến trình cũ chưa thoát, đang gửi tín hiệu SIGKILL (PID: {})...", pid);
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
        thread::sleep(Duration::from_millis(500));
    }

    !is_process_running(pid)
}

fn write_pid_file() -> std::io::Result<()> {
    let pid_path = get_pid_file_path();
    if let Some(parent) = pid_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let pid = std::process::id();
    std::fs::write(&pid_path, pid.to_string())?;
    Ok(())
}

fn cleanup_pid_file() {
    let pid_path = get_pid_file_path();
    if pid_path.exists() {
        let _ = std::fs::remove_file(pid_path);
    }
}

fn main() {
    let args = Args::parse();

    // 1. Xử lý tùy chọn dừng ứng dụng (--stop / -s)
    if args.stop {
        let pid_path = get_pid_file_path();
        if pid_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    if is_process_running(pid) {
                        if kill_process_gracefully(pid) {
                            println!("[LiveWallpaper] Đã dừng ứng dụng thành công.");
                        } else {
                            eprintln!("[LiveWallpaper] Lỗi: Không thể tắt tiến trình {}", pid);
                        }
                    } else {
                        println!("[LiveWallpaper] Tiến trình cũ (PID: {}) không còn hoạt động.", pid);
                        cleanup_pid_file();
                    }
                }
            }
        } else {
            println!("[LiveWallpaper] Không tìm thấy file PID. Không có tiến trình nào đang chạy ngầm.");
        }
        return;
    }

    // 2. Kiểm soát tiến trình chạy trùng (Single Instance)
    let pid_path = get_pid_file_path();
    if pid_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                if is_process_running(pid) && pid != std::process::id() {
                    println!("[LiveWallpaper] Phát hiện một instance khác đang chạy (PID: {}). Đang đóng để khởi chạy instance mới...", pid);
                    let _ = kill_process_gracefully(pid);
                }
            }
        }
    }

    // Ghi PID của tiến trình hiện tại
    if let Err(e) = write_pid_file() {
        eprintln!("[LiveWallpaper] Không thể tạo file PID: {}", e);
    }

    if args.daemon {
        run_daemon();
    } else {
        run_gui();
    }

    // Dọn dẹp file PID khi thoát (nếu file PID đó vẫn thuộc về tiến trình hiện tại)
    let pid_path = get_pid_file_path();
    if pid_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                if pid == std::process::id() {
                    cleanup_pid_file();
                }
            }
        }
    }
}

fn run_daemon() {
    println!("[LiveWallpaper Daemon] Đang khởi động chế độ chạy ngầm...");

    let config = AppConfig::load();
    let mut engine = WallpaperEngine::new();
    engine.apply_config(&config);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Register signal handlers for SIGINT / SIGTERM cleanup
    let _ = ctrlc::set_handler(move || {
        println!("\n[LiveWallpaper Daemon] Thao tác ngắt nhận được, đang tắt ứng dụng...");
        r.store(false, Ordering::SeqCst);
    });

    println!("[LiveWallpaper Daemon] Đang phát hình nền. Nhấn Ctrl+C để thoát.");

    while running.load(Ordering::SeqCst) {
        if config.settings.pause_on_fullscreen {
            if monitor::is_fullscreen_window_active() {
                engine.freeze_all();
            } else {
                engine.unfreeze_all();
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    engine.stop_all();
    println!("[LiveWallpaper Daemon] Đã dọn dẹp và thoát an toàn.");
}

fn run_gui() {
    let app = Application::builder()
        .application_id("com.antigravity.livewallpaper")
        .build();

    app.connect_activate(ui::build_ui);

    app.run();
}