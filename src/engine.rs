use crate::config::{AppConfig, DesktopLayerMode, DisplayConfig};
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::CStr;
use std::process::{Child, Command, Stdio};
use std::ptr;
use std::thread;
use std::time::Duration;
use crate::monitor::get_xlib;
use x11_dl::xlib;

pub fn check_dependency(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn check_dependencies() -> (bool, bool) {
    (check_dependency("xwinwrap"), check_dependency("mpv"))
}

pub fn lower_wallpaper_windows() {
    let xlib = match get_xlib() {
        Some(x) => x,
        None => return,
    };

    unsafe {
        let display = (xlib.XOpenDisplay)(ptr::null());
        if display.is_null() {
            return;
        }

        let root = (xlib.XDefaultRootWindow)(display);

        let mut root_return = 0;
        let mut parent_return = 0;
        let mut children: *mut xlib::Window = ptr::null_mut();
        let mut nchildren = 0;

        if (xlib.XQueryTree)(
            display,
            root,
            &mut root_return,
            &mut parent_return,
            &mut children,
            &mut nchildren,
        ) != 0
            && !children.is_null()
        {
            let slice = std::slice::from_raw_parts(children, nchildren as usize);
            for &win in slice {
                let mut is_target = false;

                let mut ch_name: *mut std::os::raw::c_char = ptr::null_mut();
                if (xlib.XFetchName)(display, win, &mut ch_name) != 0 && !ch_name.is_null() {
                    let name = CStr::from_ptr(ch_name).to_string_lossy();
                    (xlib.XFree)(ch_name as *mut _);
                    if name.contains("mpv") || name.contains("xwinwrap") {
                        is_target = true;
                    }
                }

                if !is_target {
                    let mut class_hint: xlib::XClassHint = std::mem::zeroed();
                    if (xlib.XGetClassHint)(display, win, &mut class_hint) != 0 {
                        let res_name = if !class_hint.res_name.is_null() {
                            let s = CStr::from_ptr(class_hint.res_name).to_string_lossy();
                            (xlib.XFree)(class_hint.res_name as *mut _);
                            s
                        } else {
                            "".into()
                        };
                        let res_class = if !class_hint.res_class.is_null() {
                            let s = CStr::from_ptr(class_hint.res_class).to_string_lossy();
                            (xlib.XFree)(class_hint.res_class as *mut _);
                            s
                        } else {
                            "".into()
                        };

                        if res_name.contains("mpv")
                            || res_class.contains("mpv")
                            || res_name.contains("xwinwrap")
                        {
                            is_target = true;
                        } else if res_class.contains("mutter-x11-frames") {
                            let mut sub_root = 0;
                            let mut sub_parent = 0;
                            let mut sub_children: *mut xlib::Window = ptr::null_mut();
                            let mut sub_nchildren = 0;

                            if (xlib.XQueryTree)(
                                display,
                                win,
                                &mut sub_root,
                                &mut sub_parent,
                                &mut sub_children,
                                &mut sub_nchildren,
                            ) != 0
                                && !sub_children.is_null()
                            {
                                let sub_slice = std::slice::from_raw_parts(
                                    sub_children,
                                    sub_nchildren as usize,
                                );
                                for &sub_win in sub_slice {
                                    let mut sub_class_hint: xlib::XClassHint = std::mem::zeroed();
                                    if (xlib.XGetClassHint)(display, sub_win, &mut sub_class_hint)
                                        != 0
                                    {
                                        if !sub_class_hint.res_class.is_null() {
                                            let s = CStr::from_ptr(sub_class_hint.res_class)
                                                .to_string_lossy();
                                            (xlib.XFree)(sub_class_hint.res_class as *mut _);
                                            if s.contains("mpv") || s.contains("gl") {
                                                is_target = true;
                                            }
                                        }
                                        if !sub_class_hint.res_name.is_null() {
                                            (xlib.XFree)(sub_class_hint.res_name as *mut _);
                                        }
                                    }
                                }
                                (xlib.XFree)(sub_children as *mut _);
                            }
                        }
                    }
                }

                if is_target {
                    (xlib.XLowerWindow)(display, win);
                }
            }
            (xlib.XFree)(children as *mut _);
        }

        (xlib.XFlush)(display);
        (xlib.XCloseDisplay)(display);
    }
}

pub struct WallpaperEngine {
    active_processes: HashMap<String, Child>,
    pub is_paused: bool,
    pub is_auto_frozen: bool,
}

impl WallpaperEngine {
    pub fn new() -> Self {
        Self {
            active_processes: HashMap::new(),
            is_paused: false,
            is_auto_frozen: false,
        }
    }

    pub fn stop_display(&mut self, name: &str) {
        if let Some(mut child) = self.active_processes.remove(name) {
            let _ = child.kill();
            let _ = child.wait();
            println!("[Engine] Đã dừng wallpaper cho màn hình [{}]", name);
        }
    }

    pub fn stop_all(&mut self) {
        let keys: Vec<String> = self.active_processes.keys().cloned().collect();
        for key in keys {
            self.stop_display(&key);
        }
        self.is_auto_frozen = false;
    }

    pub fn freeze_all(&mut self) {
        if self.is_auto_frozen || self.is_paused {
            return;
        }
        for child in self.active_processes.values() {
            let pid = child.id().to_string();
            let _ = Command::new("kill").args(["-STOP", &pid]).status();
            let _ = Command::new("pkill").args(["-P", &pid, "-STOP"]).status();
        }
        self.is_auto_frozen = true;
        println!("[Engine] Đã tự động đóng băng (SIGSTOP) wallpaper do ứng dụng Fullscreen.");
    }

    pub fn unfreeze_all(&mut self) {
        if !self.is_auto_frozen || self.is_paused {
            return;
        }
        for child in self.active_processes.values() {
            let pid = child.id().to_string();
            let _ = Command::new("pkill").args(["-P", &pid, "-CONT"]).status();
            let _ = Command::new("kill").args(["-CONT", &pid]).status();
        }
        self.is_auto_frozen = false;
        println!("[Engine] Đã tiếp tục phát (SIGCONT) wallpaper sau khi thoát Fullscreen.");
    }

    pub fn start_display(
        &mut self,
        display: &DisplayConfig,
        layer_mode: DesktopLayerMode,
    ) -> Result<()> {
        self.stop_display(&display.name);

        if !display.enabled || display.video_path.trim().is_empty() {
            return Ok(());
        }

        if !std::path::Path::new(&display.video_path).exists() {
            eprintln!(
                "[Engine] File video không tồn tại cho [{}]: {}",
                display.name, display.video_path
            );
            return Ok(());
        }

        println!(
            "[Engine] Đang phát video (HW Acceleration) cho [{}] ({}) -> {}",
            display.name, display.geometry, display.video_path
        );

        let mut mpv_args = vec![
            "-wid".to_string(),
            "WID".to_string(),
            "--loop=inf".to_string(),
            "--no-osc".to_string(),
            "--no-osd-bar".to_string(),
            "--no-input-default-bindings".to_string(),
            "--hwdec=auto-safe".to_string(),
            "--vo=gpu".to_string(),
            "--gpu-context=auto".to_string(),
            "--opengl-swapinterval=0".to_string(),
            "--vulkan-swap-mode=immediate".to_string(),
            "--scale=bilinear".to_string(),
            "--cscale=bilinear".to_string(),
            "--dscale=bilinear".to_string(),
            "--correct-downscaling=no".to_string(),
            "--framedrop=vo".to_string(),
            "--vd-lavc-fast".to_string(),
            "--demuxer-max-bytes=10M".to_string(),
            "--demuxer-readahead-secs=2".to_string(),
            "--no-audio-display".to_string(),
        ];

        if display.mute {
            mpv_args.push("--no-audio".to_string());
        } else {
            mpv_args.push(format!("--volume={}", display.volume));
        }

        mpv_args.push(display.video_path.clone());

        let mut xwinwrap_args = vec![
            "-g".to_string(),
            display.geometry.clone(),
            "-ni".to_string(),
            "-b".to_string(),
            "-nf".to_string(),
            "-un".to_string(),
            "-s".to_string(),
            "-st".to_string(),
            "-sp".to_string(),
        ];

        match layer_mode {
            DesktopLayerMode::Standard => {}
            DesktopLayerMode::OverrideRedirect => {
                xwinwrap_args.push("-ov".to_string());
            }
            DesktopLayerMode::ForceDesktopType => {
                xwinwrap_args.push("-fdt".to_string());
            }
        }

        xwinwrap_args.push("--".to_string());
        xwinwrap_args.push("mpv".to_string());

        xwinwrap_args.extend(mpv_args);

        let child = if check_dependency("nice") {
            let mut nice_args = vec!["-n".to_string(), "10".to_string(), "xwinwrap".to_string()];
            nice_args.extend(xwinwrap_args);
            Command::new("nice")
                .args(&nice_args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?
        } else {
            Command::new("xwinwrap")
                .args(&xwinwrap_args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?
        };

        self.active_processes.insert(display.name.clone(), child);

        // Lower the wallpaper window in background after window manager creates it
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(500));
            lower_wallpaper_windows();
        });

        Ok(())
    }

    pub fn apply_config(&mut self, config: &AppConfig) {
        if self.is_paused {
            return;
        }

        for display in &config.displays {
            let _ = self.start_display(display, config.settings.layer_mode);
        }

        if self.is_auto_frozen {
            self.is_auto_frozen = false;
            self.freeze_all();
        }
    }

    pub fn toggle_pause(&mut self, config: &AppConfig) -> bool {
        self.is_paused = !self.is_paused;
        if self.is_paused {
            self.stop_all();
            println!("[Engine] Đã tạm dừng tất cả wallpaper");
        } else {
            self.apply_config(config);
            println!("[Engine] Đã tiếp tục phát wallpaper");
        }
        self.is_paused
    }

    pub fn active_count(&self) -> usize {
        self.active_processes.len()
    }
}

