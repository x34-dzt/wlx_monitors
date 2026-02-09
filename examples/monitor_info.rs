use std::sync::mpsc;

use wlx_monitors::{WlMonitorEvent, WlMonitorManager};

fn main() {
    let (event_tx, event_rx) = mpsc::sync_channel(16);
    let (_action_tx, action_rx) = mpsc::sync_channel(16);

    let (state, event_queue) =
        WlMonitorManager::new_connection(event_tx, action_rx)
            .expect("Failed to connect to Wayland");

    std::thread::spawn(move || {
        state.run(event_queue).expect("Event loop error");
    });

    while let Ok(event) = event_rx.recv() {
        match event {
            WlMonitorEvent::InitialState(monitors) => {
                println!("=== {} monitors detected ===\n", monitors.len());
                for monitor in &monitors {
                    println!("  {} ({})", monitor.name, monitor.description);
                    println!("    enabled: {}", monitor.enabled);
                    println!(
                        "    position: ({}, {})",
                        monitor.position.x, monitor.position.y
                    );
                    println!("    scale: {}", monitor.scale);
                    println!("    modes:");
                    for mode in &monitor.modes {
                        let mut flags = String::new();
                        if mode.preferred {
                            flags.push_str(" (preferred)");
                        }
                        if mode.is_current {
                            flags.push_str(" [CURRENT]");
                        }
                        println!(
                            "      {}x{} @ {}Hz{}",
                            mode.resolution.width,
                            mode.resolution.height,
                            mode.refresh_rate,
                            flags,
                        );
                    }
                    println!();
                }
            }
            WlMonitorEvent::Changed(monitor) => {
                println!("=== changed: {} ===", monitor.name);
                println!("    enabled: {}", monitor.enabled);
                println!();
            }
            WlMonitorEvent::Removed { name, .. } => {
                println!("=== removed: {} ===\n", name);
            }
        }
    }
}
