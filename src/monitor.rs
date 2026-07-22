use gtk::gdk::prelude::*;
use std::process::Command;
use std::ptr;
use x11_dl::xlib::{self, Xlib};

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub name: String,
    pub geometry: String,
}

pub fn is_fullscreen_window_active() -> bool {
    let xlib = match Xlib::open() {
        Ok(x) => x,
        Err(_) => return false,
    };

    unsafe {
        let display = (xlib.XOpenDisplay)(ptr::null());
        if display.is_null() {
            return false;
        }

        let root = (xlib.XDefaultRootWindow)(display);

        let net_active_win = (xlib.XInternAtom)(
            display,
            b"_NET_ACTIVE_WINDOW\0".as_ptr() as *const _,
            xlib::False,
        );
        let net_wm_state = (xlib.XInternAtom)(
            display,
            b"_NET_WM_STATE\0".as_ptr() as *const _,
            xlib::False,
        );
        let net_wm_state_fullscreen = (xlib.XInternAtom)(
            display,
            b"_NET_WM_STATE_FULLSCREEN\0".as_ptr() as *const _,
            xlib::False,
        );

        if net_active_win == 0 || net_wm_state == 0 || net_wm_state_fullscreen == 0 {
            (xlib.XCloseDisplay)(display);
            return false;
        }

        let mut actual_type = 0;
        let mut actual_format = 0;
        let mut nitems = 0;
        let mut bytes_after = 0;
        let mut prop: *mut u8 = ptr::null_mut();

        let status = (xlib.XGetWindowProperty)(
            display,
            root,
            net_active_win,
            0,
            1,
            xlib::False,
            xlib::XA_WINDOW,
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut prop,
        );

        let active_win = if status == 0 && !prop.is_null() && nitems > 0 {
            let win = *(prop as *const xlib::Window);
            (xlib.XFree)(prop as *mut _);
            win
        } else {
            if !prop.is_null() {
                (xlib.XFree)(prop as *mut _);
            }
            (xlib.XCloseDisplay)(display);
            return false;
        };

        if active_win == 0 || active_win == root {
            (xlib.XCloseDisplay)(display);
            return false;
        }

        let mut prop_state: *mut u8 = ptr::null_mut();
        let status_state = (xlib.XGetWindowProperty)(
            display,
            active_win,
            net_wm_state,
            0,
            1024,
            xlib::False,
            xlib::XA_ATOM,
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut prop_state,
        );

        let mut is_fullscreen = false;
        if status_state == 0 && !prop_state.is_null() && nitems > 0 {
            let atoms = std::slice::from_raw_parts(prop_state as *const xlib::Atom, nitems as usize);
            for &atom in atoms {
                if atom == net_wm_state_fullscreen {
                    is_fullscreen = true;
                    break;
                }
            }
            (xlib.XFree)(prop_state as *mut _);
        } else if !prop_state.is_null() {
            (xlib.XFree)(prop_state as *mut _);
        }

        (xlib.XCloseDisplay)(display);
        is_fullscreen
    }
}


pub fn detect_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    // Strategy 1: Try GTK / GDK Display monitors if GTK is initialized
    if let Some(display) = gtk::gdk::Display::default() {
        let list_model = display.monitors();
        let n = list_model.n_items();
        for i in 0..n {
            if let Some(obj) = list_model.item(i) {
                if let Ok(mon) = obj.downcast::<gtk::gdk::Monitor>() {
                    let connector = mon
                        .connector()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("Display-{}", i + 1));
                    let geom = mon.geometry();
                    let geometry_str =
                        format!("{}x{}+{}+{}", geom.width(), geom.height(), geom.x(), geom.y());
                    monitors.push(MonitorInfo {
                        name: connector,
                        geometry: geometry_str,
                    });
                }
            }
        }
    }

    // Strategy 2: Fallback to xrandr CLI if GDK yielded no monitors
    if monitors.is_empty() {
        if let Ok(output) = Command::new("xrandr").arg("--query").output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    // Example line: "HDMI-1 connected 1920x1080+0+0 (normal left inverted right x axis y axis)"
                    if line.contains(" connected ") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            let name = parts[0].to_string();
                            // find geometry matching pattern WxH+X+Y
                            for part in &parts[1..] {
                                if part.contains('x') && part.contains('+') {
                                    monitors.push(MonitorInfo {
                                        name: name.clone(),
                                        geometry: part.to_string(),
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Strategy 3: Ultimate default fallback if all fails
    if monitors.is_empty() {
        monitors.push(MonitorInfo {
            name: "HDMI-1".to_string(),
            geometry: "1920x1080+0+0".to_string(),
        });
    }

    monitors
}
