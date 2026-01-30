# wayland-monitor-detector

Detects connected monitors on Wayland using the `wlr-output-management` protocol.

## Requirements

- Wayland compositor with `zwlr_output_manager_v1` support (wlroots-based compositors like Sway, Hyprland, etc.)

## Build & Run

```bash
cargo build --release
cargo run
```

## How it works

1. Connects to Wayland display via `WAYLAND_DISPLAY` env
2. Binds to `zwlr_output_manager_v1` global
3. Receives `head` events for each monitor
4. Prints monitor name, resolution, and refresh rate

## Protocol References

- [wl_registry](https://gitlab.freedesktop.org/wayland/wayland/-/blob/main/protocol/wayland.xml#L71)
- [wlr-output-management-unstable-v1](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-output-management-unstable-v1.xml)
