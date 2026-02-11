# wlx_monitors

A Rust library for detecting and managing display outputs on Wayland using the `wlr-output-management` protocol.

[![Crates.io](https://img.shields.io/crates/v/wlx_monitors)](https://crates.io/crates/wlx_monitors)
[![Documentation](https://docs.rs/wlx_monitors/badge.svg)](https://docs.rs/wlx_monitors)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## What is this?

`wlx_monitors` provides a safe, idiomatic Rust interface to:

- **Detect** connected monitors and their properties (resolution, refresh rate, position, scale)
- **Monitor** for display hotplug events (monitor connected/disconnected)
- **Control** display outputs (enable/disable, change resolution/refresh rate, scale, and transform/rotation)

Works with wlroots-based Wayland compositors (Sway, Hyprland, River, dwl, etc.) that implement the `zwlr_output_manager_v1` protocol.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
wlx_monitors = "0.1.3"
```

Basic usage:

```rust
use wlx_monitors::{WlMonitorManager, WlMonitorEvent};
use std::sync::mpsc;

fn main() {
    // Create channels for event communication
    let (event_tx, event_rx) = mpsc::sync_channel(16);
    let (action_tx, action_rx) = mpsc::sync_channel(16);
    
    // Connect to Wayland
    let (manager, event_queue) = WlMonitorManager::new_connection(
        event_tx, 
        action_rx
    ).expect("Failed to connect to Wayland");
    
    // Run the event loop in a separate thread
    std::thread::spawn(move || {
        manager.run(event_queue).expect("Event loop error");
    });
    
    // Process monitor events
    while let Ok(event) = event_rx.recv() {
        match event {
            WlMonitorEvent::InitialState(monitors) => {
                println!("Detected {} monitors", monitors.len());
                for monitor in monitors {
                    println!("  {} - {}x{}", 
                        monitor.name,
                        monitor.resolution.width,
                        monitor.resolution.height
                    );
                }
            }
            WlMonitorEvent::Changed(monitor) => {
                println!("Monitor {} changed", monitor.name);
            }
            WlMonitorEvent::Removed { name, .. } => {
                println!("Monitor {} disconnected", name);
            }
            WlMonitorEvent::ActionFailed { action, reason } => {
                eprintln!("Action {:?} failed: {}", action, reason);
            }
        }
    }
}
```

Run the included example:

```bash
cargo run --example monitor_info
```

## Architecture

This library uses a **channel-based event loop** pattern:

### Events (Wayland → Your App)

The library sends events through an MPSC channel:

- `WlMonitorEvent::InitialState(Vec<WlMonitor>)` - Sent once with all currently connected monitors
- `WlMonitorEvent::Changed(Box<WlMonitor>)` - Sent when a monitor's properties change
- `WlMonitorEvent::Removed { id, name }` - Sent when a monitor is disconnected
- `WlMonitorEvent::ActionFailed { action, reason }` - Sent when an action fails (e.g., invalid mode)

### Actions (Your App → Wayland)

Send control actions through another MPSC channel:

- `WlMonitorAction::Toggle { name, mode }` - Enable/disable a monitor by name. The `mode: Option<(i32, i32, i32)>` lets users optionally specify a custom `(width, height, refresh_rate)` when toggling a monitor back on. If `None`, the smart mode resolution kicks in (last mode > preferred > first available).
- `WlMonitorAction::SwitchMode { name, width, height, refresh_rate }` - Change a monitor's mode
- `WlMonitorAction::SetScale { name, scale }` - Set a monitor's scale factor (must be > 0, e.g., 1.0, 1.5, 2.0)
- `WlMonitorAction::SetTransform { name, transform }` - Set a monitor's rotation/orientation (Normal, Rotate90, Rotate180, Rotate270, Flipped, etc.)

### Threading Model

```
┌─────────────────┐     events      ┌──────────────────┐
│  Wayland Server │ ───────────────>│   Your App       │
│  (Compositor)   │                 │  (Main Thread)   │
└─────────────────┘                 └──────────────────┘
         ^                                    │
         │                                   │
         │          actions                   │
         └────────────────────────────────────┘
         
┌─────────────────┐
│ Event Loop      │
│ (Separate       │
│  Thread)        │
└─────────────────┘
```

## API Overview

### Core Types

- **`WlMonitorManager`** - Main entry point. Manages the Wayland connection and event loop.
- **`WlMonitor`** - Represents a connected display with properties (name, resolution, modes, etc.)
- **`WlMonitorMode`** - A display mode (resolution + refresh rate)
- **`WlResolution`** / **`WlPosition`** - Basic geometry types

### Events

```rust
pub enum WlMonitorEvent {
    InitialState(Vec<WlMonitor>),           // All monitors at startup
    Changed(Box<WlMonitor>),                // Monitor properties changed
    Removed { id: ObjectId, name: String }, // Monitor disconnected
    ActionFailed { action: ActionKind, reason: String }, // Action failed
}
```

### Actions

```rust
pub enum WlMonitorAction {
    Toggle { name: String, mode: Option<(i32, i32, i32)> },    // On/off with optional custom mode
    SwitchMode { name: String, width: i32, height: i32, refresh_rate: i32 },
    SetScale { name: String, scale: f64 },                      // Set scale factor
    SetTransform { name: String, transform: Transform },        // Set rotation/flip
}
```

## Monitor Properties

Each `WlMonitor` provides:

| Property | Type | Description |
|----------|------|-------------|
| `name` | `String` | Output name (e.g., "DP-1", "HDMI-A-1") |
| `description` | `String` | Human-readable description |
| `make` | `String` | Manufacturer |
| `model` | `String` | Model name |
| `serial_number` | `String` | Serial number |
| `enabled` | `bool` | Currently enabled? |
| `resolution` | `WlResolution` | Current resolution (width, height) |
| `position` | `WlPosition` | Position in global coordinate space |
| `scale` | `f64` | Scale factor (1.0, 1.5, 2.0, etc.) |
| `modes` | `Vec<WlMonitorMode>` | Available display modes |
| `transform` | `Transform` | Orientation (normal, rotated, flipped) |

## Requirements

- **Wayland compositor** with `zwlr_output_manager_v1` support:
  - ✓ wlroots-based compositors (Sway, Hyprland, River, dwl, Wayfire, etc.)
  - ✓ Some other compositors may support this protocol
  - ✗ GNOME (uses different protocol)
  - ✗ KDE Plasma (uses different protocol)

- **Rust 1.85+** (for Edition 2024)

## Building

```bash
# Clone the repository
git clone https://github.com/x34-dzt/wlx_monitors
cd wlx_monitors

# Build
cargo build --release

# Run example
cargo run --example monitor_info
```

## Example: Controlling Monitors

```rust
use wlx_monitors::{WlMonitorManager, WlMonitorEvent, WlMonitorAction};
use std::sync::mpsc;
use std::thread;

fn main() {
    let (event_tx, event_rx) = mpsc::sync_channel(16);
    let (action_tx, action_rx) = mpsc::sync_channel(16);
    
    let (manager, event_queue) = WlMonitorManager::new_connection(
        event_tx, 
        action_rx
    ).unwrap();
    
    // Spawn event loop
    thread::spawn(move || {
        manager.run(event_queue).unwrap();
    });
    
    // Example: Toggle a monitor
    action_tx.send(WlMonitorAction::Toggle {
        name: "DP-1".to_string(),
        mode: None,
    }).unwrap();
    
    // Example: Switch resolution
    action_tx.send(WlMonitorAction::SwitchMode {
        name: "HDMI-A-1".to_string(),
        width: 1920,
        height: 1080,
        refresh_rate: 60,
    }).unwrap();

    // Example: Set scale factor
    action_tx.send(WlMonitorAction::SetScale {
        name: "DP-1".to_string(),
        scale: 1.5,
    }).unwrap();

    // Example: Rotate a monitor
    use wlx_monitors::Transform;
    action_tx.send(WlMonitorAction::SetTransform {
        name: "DP-1".to_string(),
        transform: Transform::Rotate90,
    }).unwrap();

    // Process events
    while let Ok(event) = event_rx.recv() {
        match event {
            WlMonitorEvent::Changed(monitor) => {
                println!("Updated: {} - enabled={}", 
                    monitor.name, 
                    monitor.enabled
                );
            }
            _ => {}
        }
    }
}
```

## Troubleshooting

### "Failed to connect to Wayland"

Make sure you're running on a Wayland session:
```bash
echo $WAYLAND_DISPLAY
# Should output something like "wayland-1"
```

### "Compositor rejected the configuration"

The compositor may not support the requested mode or the monitor doesn't support the requested resolution/refresh rate.

## Protocol References

- [wlr-output-management-unstable-v1](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-output-management-unstable-v1.xml)
- [wlroots output management protocol docs](https://wayland.app/protocols/wlr-output-management-unstable-v1)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
