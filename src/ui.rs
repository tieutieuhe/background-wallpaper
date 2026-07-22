use crate::autostart::{is_autostart_enabled, set_autostart};
use crate::config::{AppConfig, DisplayConfig};
use crate::engine::{check_dependencies, WallpaperEngine};
use crate::monitor::{detect_monitors, is_fullscreen_window_active};
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box, Button, CheckButton, DropDown, FileChooserAction,
    FileChooserDialog, FileFilter, HeaderBar, Label, Orientation, ResponseType, Scale,
    ScrolledWindow, Switch,
};


use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn build_ui(app: &Application) {
    let (has_xwinwrap, has_mpv) = check_dependencies();

    let config = Rc::new(RefCell::new(AppConfig::load()));
    let engine = Rc::new(RefCell::new(WallpaperEngine::new()));
    let is_exiting = Rc::new(Cell::new(false));

    // Apply config to monitors initially
    {
        let detected = detect_monitors();
        let mut cfg = config.borrow_mut();
        for mon in detected {
            cfg.update_or_add_display(DisplayConfig {
                name: mon.name,
                geometry: mon.geometry,
                video_path: "".to_string(),
                enabled: true,
                mute: true,
                volume: 100,
            });
        }
        let _ = cfg.save();
    }

    // Start wallpapers
    engine.borrow_mut().apply_config(&config.borrow());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Live Wallpaper Engine")
        .default_width(760)
        .default_height(560)
        .build();

    let header_bar = HeaderBar::new();

    let refresh_btn = Button::from_icon_name("view-refresh-symbolic");
    refresh_btn.set_tooltip_text(Some("Cập nhật danh sách màn hình"));
    header_bar.pack_start(&refresh_btn);

    let exit_btn = Button::from_icon_name("application-exit-symbolic");
    exit_btn.set_tooltip_text(Some("Thoát hoàn toàn ứng dụng và dừng tất cả hình nền"));
    header_bar.pack_end(&exit_btn);

    let pause_btn = Button::from_icon_name("media-playback-pause-symbolic");
    pause_btn.set_tooltip_text(Some("Tạm dừng / Tiếp tục tất cả hình nền"));
    header_bar.pack_end(&pause_btn);

    window.set_titlebar(Some(&header_bar));

    let main_vbox = Box::new(Orientation::Vertical, 16);
    main_vbox.set_margin_start(16);
    main_vbox.set_margin_end(16);
    main_vbox.set_margin_top(16);
    main_vbox.set_margin_bottom(16);

    // Dependency warning banner if missing
    if !has_xwinwrap || !has_mpv {
        let warn_box = Box::new(Orientation::Horizontal, 12);
        warn_box.set_margin_bottom(8);

        let warn_label = Label::new(Some(
            "⚠️ CẢNH BÁO: Hệ thống chưa cài đặt xwinwrap hoặc mpv!\n\
             Vui lòng chạy lệnh: sudo apt install mpv xwinwrap (hoặc yay -S xwinwrap-git mpv)",
        ));
        warn_label.set_wrap(true);
        warn_box.append(&warn_label);
        main_vbox.append(&warn_box);
    }

    // Status Label
    let status_label = Label::new(Some("⚡ Đang hoạt động bình thường"));
    status_label.set_halign(Align::Start);
    main_vbox.append(&status_label);

    // Scrolled window for monitors container
    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);

    let monitors_vbox = Box::new(Orientation::Vertical, 16);
    scrolled.set_child(Some(&monitors_vbox));
    main_vbox.append(&scrolled);

    // Build Monitor Cards function
    let rebuild_monitor_cards = {
        let monitors_vbox = monitors_vbox.clone();
        let config = config.clone();
        let engine = engine.clone();
        let window_clone = window.clone();
        let status_label_clone = status_label.clone();

        Rc::new(move || {
            // Clear existing children
            while let Some(child) = monitors_vbox.first_child() {
                monitors_vbox.remove(&child);
            }

            let cfg = config.borrow();
            let active_count = engine.borrow().active_count();
            let is_paused = engine.borrow().is_paused;

            if is_paused {
                status_label_clone.set_text("⏸️ Trạng thái: Đã tạm dừng tất cả hình nền");
            } else {
                status_label_clone.set_text(&format!(
                    "⚡ Trạng thái: Đang phát wallpaper trên {} màn hình",
                    active_count
                ));
            }

            for disp in &cfg.displays {
                let card = Box::new(Orientation::Vertical, 12);
                card.set_margin_start(8);
                card.set_margin_end(8);
                card.set_margin_top(8);
                card.set_margin_bottom(8);

                // Title row
                let title_row = Box::new(Orientation::Horizontal, 12);

                let title_label = Label::new(Some(&format!(
                    "🖥️ Màn hình: {} [{}]",
                    disp.name, disp.geometry
                )));
                title_label.set_halign(Align::Start);
                title_label.set_hexpand(true);

                let enable_switch = Switch::new();
                enable_switch.set_active(disp.enabled);
                enable_switch.set_tooltip_text(Some("Bật/Tắt hình nền trên màn hình này"));

                title_row.append(&title_label);
                title_row.append(&enable_switch);
                card.append(&title_row);

                // File Path row
                let file_row = Box::new(Orientation::Horizontal, 8);

                let file_label = Label::new(Some(if disp.video_path.is_empty() {
                    "Chưa chọn file video..."
                } else {
                    &disp.video_path
                }));
                file_label.set_halign(Align::Start);
                file_label.set_hexpand(true);
                file_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);

                let choose_btn = Button::with_label("📂 Chọn Video...");

                file_row.append(&file_label);
                file_row.append(&choose_btn);
                card.append(&file_row);

                // Audio controls row
                let audio_row = Box::new(Orientation::Horizontal, 12);

                let mute_check = CheckButton::with_label("Tắt tiếng (Mute)");
                mute_check.set_active(disp.mute);

                let vol_label = Label::new(Some("Âm lượng:"));
                let vol_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
                vol_scale.set_value(disp.volume as f64);
                vol_scale.set_hexpand(true);
                vol_scale.set_sensitive(!disp.mute);

                audio_row.append(&mute_check);
                audio_row.append(&vol_label);
                audio_row.append(&vol_scale);
                card.append(&audio_row);

                // Event Handlers for Card
                let disp_name = disp.name.clone();
                let config_inner = config.clone();
                let engine_inner = engine.clone();

                // Toggle Enable Switch
                let disp_name_cb = disp_name.clone();
                let config_cb = config_inner.clone();
                let engine_cb = engine_inner.clone();
                enable_switch.connect_state_set(move |_, state| {
                    let mut cfg = config_cb.borrow_mut();
                    if let Some(d) = cfg.get_display_mut(&disp_name_cb) {
                        d.enabled = state;
                    }
                    let _ = cfg.save();
                    engine_cb.borrow_mut().apply_config(&cfg);
                    glib::Propagation::Proceed
                });

                // Choose Video Button
                let disp_name_cb = disp_name.clone();
                let config_cb = config_inner.clone();
                let engine_cb = engine_inner.clone();
                let parent_win = window_clone.clone();
                let file_label_cb = file_label.clone();
                choose_btn.connect_clicked(move |_| {
                    let dialog = FileChooserDialog::new(
                        Some("Chọn Video làm Live Wallpaper"),
                        Some(&parent_win),
                        FileChooserAction::Open,
                        &[
                            ("_Hủy", ResponseType::Cancel),
                            ("_Mở", ResponseType::Accept),
                        ],
                    );
                    dialog.set_modal(true);

                    let filter = FileFilter::new();
                    filter.set_name(Some("Video Files (*.mp4, *.mkv, *.webm, *.mov, *.avi)"));
                    filter.add_mime_type("video/*");
                    filter.add_pattern("*.mp4");
                    filter.add_pattern("*.mkv");
                    filter.add_pattern("*.webm");
                    filter.add_pattern("*.mov");
                    filter.add_pattern("*.avi");

                    dialog.add_filter(&filter);

                    let disp_name_dlg = disp_name_cb.clone();
                    let config_dlg = config_cb.clone();
                    let engine_dlg = engine_cb.clone();
                    let file_label_dlg = file_label_cb.clone();

                    dialog.connect_response(move |dialog_res, response| {
                        if response == ResponseType::Accept {
                            if let Some(file) = dialog_res.file() {
                                if let Some(path) = file.path() {
                                    let path_str = path.to_string_lossy().to_string();
                                    file_label_dlg.set_text(&path_str);

                                    let mut cfg = config_dlg.borrow_mut();
                                    if let Some(d) = cfg.get_display_mut(&disp_name_dlg) {
                                        d.video_path = path_str;
                                    }
                                    let _ = cfg.save();
                                    engine_dlg.borrow_mut().apply_config(&cfg);
                                }
                            }
                        }
                        dialog_res.destroy();
                    });

                    dialog.show();
                });


                // Mute Checkbox
                let disp_name_cb = disp_name.clone();
                let config_cb = config_inner.clone();
                let engine_cb = engine_inner.clone();
                let vol_scale_cb = vol_scale.clone();
                mute_check.connect_toggled(move |btn| {
                    let is_muted = btn.is_active();
                    vol_scale_cb.set_sensitive(!is_muted);

                    let mut cfg = config_cb.borrow_mut();
                    if let Some(d) = cfg.get_display_mut(&disp_name_cb) {
                        d.mute = is_muted;
                    }
                    let _ = cfg.save();
                    engine_cb.borrow_mut().apply_config(&cfg);
                });

                // Volume Scale Slider
                let disp_name_cb = disp_name.clone();
                let config_cb = config_inner.clone();
                let engine_cb = engine_inner.clone();
                vol_scale.connect_value_changed(move |scale| {
                    let val = scale.value() as u32;
                    let mut cfg = config_cb.borrow_mut();
                    if let Some(d) = cfg.get_display_mut(&disp_name_cb) {
                        d.volume = val;
                    }
                    let _ = cfg.save();
                    engine_cb.borrow_mut().apply_config(&cfg);
                });

                monitors_vbox.append(&card);
            }
        })
    };

    // Initial build of cards
    rebuild_monitor_cards();

    // Autostart Option Box
    let autostart_box = Box::new(Orientation::Horizontal, 12);
    autostart_box.set_margin_top(8);

    let autostart_label = Label::new(Some("🚀 Tự khởi động cùng hệ thống (Autostart)"));
    autostart_label.set_halign(Align::Start);
    autostart_label.set_hexpand(true);

    let autostart_switch = Switch::new();
    autostart_switch.set_active(is_autostart_enabled());

    autostart_switch.connect_state_set(move |_, state| {
        let _ = set_autostart(state);
        glib::Propagation::Proceed
    });

    autostart_box.append(&autostart_label);
    autostart_box.append(&autostart_switch);
    main_vbox.append(&autostart_box);

    // Pause on Fullscreen Option Box
    let fullscreen_box = Box::new(Orientation::Horizontal, 12);
    fullscreen_box.set_margin_top(4);

    let fullscreen_label = Label::new(Some("⏸️ Tự động tạm dừng khi ứng dụng Toàn màn hình (Fullscreen)"));
    fullscreen_label.set_halign(Align::Start);
    fullscreen_label.set_hexpand(true);

    let fullscreen_switch = Switch::new();
    fullscreen_switch.set_active(config.borrow().settings.pause_on_fullscreen);

    let config_fs = config.clone();
    let engine_fs = engine.clone();
    fullscreen_switch.connect_state_set(move |_, state| {
        let mut cfg = config_fs.borrow_mut();
        cfg.settings.pause_on_fullscreen = state;
        let _ = cfg.save();
        if !state {
            engine_fs.borrow_mut().unfreeze_all();
        }
        glib::Propagation::Proceed
    });

    fullscreen_box.append(&fullscreen_label);
    fullscreen_box.append(&fullscreen_switch);
    main_vbox.append(&fullscreen_box);

    // Layer Mode Setting Box
    let layer_box = Box::new(Orientation::Horizontal, 12);
    layer_box.set_margin_top(4);

    let layer_label = Label::new(Some("🪟 Chế độ hiển thị (Tương thích Icon Desktop):"));
    layer_label.set_halign(Align::Start);
    layer_label.set_hexpand(true);

    let layer_options = [
        "Dưới biểu tượng Desktop (Standard)",
        "Tùy biến X11 (Override Redirect)",
        "Phủ lên nền (Force Desktop Type)",
    ];
    let layer_dropdown = DropDown::from_strings(&layer_options);

    let current_layer = config.borrow().settings.layer_mode;
    match current_layer {
        crate::config::DesktopLayerMode::Standard => layer_dropdown.set_selected(0),
        crate::config::DesktopLayerMode::OverrideRedirect => layer_dropdown.set_selected(1),
        crate::config::DesktopLayerMode::ForceDesktopType => layer_dropdown.set_selected(2),
    }

    let config_layer = config.clone();
    let engine_layer = engine.clone();
    layer_dropdown.connect_selected_notify(move |dropdown| {
        let sel = dropdown.selected();
        let mode = match sel {
            0 => crate::config::DesktopLayerMode::Standard,
            1 => crate::config::DesktopLayerMode::OverrideRedirect,
            _ => crate::config::DesktopLayerMode::ForceDesktopType,
        };

        let mut cfg = config_layer.borrow_mut();
        cfg.settings.layer_mode = mode;
        let _ = cfg.save();
        engine_layer.borrow_mut().apply_config(&cfg);
    });

    layer_box.append(&layer_label);
    layer_box.append(&layer_dropdown);
    main_vbox.append(&layer_box);


    // Button Events
    let rebuild_cards_ref = rebuild_monitor_cards.clone();
    refresh_btn.connect_clicked(move |_| {
        rebuild_cards_ref();
    });

    let config_pause = config.clone();
    let engine_pause = engine.clone();
    let pause_btn_clone = pause_btn.clone();
    let rebuild_cards_pause = rebuild_monitor_cards.clone();
    pause_btn.connect_clicked(move |_| {
        let is_paused = engine_pause
            .borrow_mut()
            .toggle_pause(&config_pause.borrow());
        if is_paused {
            pause_btn_clone.set_icon_name("media-playback-start-symbolic");
            pause_btn_clone.set_tooltip_text(Some("Tiếp tục phát hình nền"));
        } else {
            pause_btn_clone.set_icon_name("media-playback-pause-symbolic");
            pause_btn_clone.set_tooltip_text(Some("Tạm dừng hình nền"));
        }
        rebuild_cards_pause();
    });

    window.set_child(Some(&main_vbox));

    // Xử lý sự kiện click của nút Thoát hoàn toàn
    let app_exit = app.clone();
    let engine_exit = engine.clone();
    let is_exiting_btn = is_exiting.clone();
    exit_btn.connect_clicked(move |_| {
        println!("[GUI] Người dùng yêu cầu thoát hoàn toàn ứng dụng...");
        is_exiting_btn.set(true);
        engine_exit.borrow_mut().stop_all();

        let pid_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("livewallpaper")
            .join("livewallpaper.pid");
        if pid_path.exists() {
            let _ = std::fs::remove_file(pid_path);
        }

        app_exit.quit();
    });

    // Xử lý sự kiện đóng cửa sổ (nút X)
    let engine_close = engine.clone();
    let is_exiting_close = is_exiting.clone();
    window.connect_close_request(move |_| {
        if is_exiting_close.get() {
            engine_close.borrow_mut().stop_all();
        } else {
            println!("[GUI] Cửa sổ đóng, đang chuyển sang chế độ chạy ngầm...");
            engine_close.borrow_mut().stop_all();

            let pid_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("livewallpaper")
                .join("livewallpaper.pid");
            if pid_path.exists() {
                let _ = std::fs::remove_file(pid_path);
            }

            if let Ok(current_exe) = std::env::current_exe() {
                if let Err(e) = std::process::Command::new(current_exe)
                    .arg("--daemon")
                    .spawn()
                {
                    eprintln!("[GUI] Lỗi khi khởi động daemon ngầm: {}", e);
                } else {
                    println!("[GUI] Đã khởi chạy chế độ chạy ngầm.");
                }
            }
        }
        glib::Propagation::Proceed
    });

    // Register background timeout to monitor fullscreen status
    let config_mon = config.clone();
    let engine_mon = engine.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(1000), move || {
        let pause_fs = config_mon.borrow().settings.pause_on_fullscreen;
        if pause_fs {
            if is_fullscreen_window_active() {
                engine_mon.borrow_mut().freeze_all();
            } else {
                engine_mon.borrow_mut().unfreeze_all();
            }
        }
        glib::ControlFlow::Continue
    });

    window.present();
}
