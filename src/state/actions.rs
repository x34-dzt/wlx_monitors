use wayland_client::{EventQueue, Proxy, QueueHandle, backend::ObjectId};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_configuration_v1::ZwlrOutputConfigurationV1,
};

use crate::wl_monitor::{WlMonitor, WlTransform};

use super::{WlMonitorManager, WlMonitorManagerError};

/// The kind of action that failed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    Toggle,
    ConfigApply,
    SwitchMode,
    SetScale,
    SetTransform,
    SetPosition,
}

/// Events emitted by the Wayland monitor manager
#[derive(Debug, Clone)]
pub enum WlMonitorEvent {
    /// Sent once when the initial state is received, containing all connected monitors
    InitialState(Vec<WlMonitor>),
    /// Sent when a monitor's properties have changed
    Changed(Box<WlMonitor>),
    /// Sent when a monitor is disconnected
    Removed { id: ObjectId, name: String },
    /// Sent when an action fails (e.g., invalid mode specified)
    ActionFailed { action: ActionKind, reason: String },
}

/// Actions that can be sent to the monitor manager to control monitors
#[derive(Debug, Clone)]
pub enum WlMonitorAction {
    /// Toggle a monitor on/off by name
    Toggle {
        /// Name of the monitor to toggle (e.g., "DP-1")
        name: String,
        /// Optional custom mode: (width, height, refresh_rate)
        mode: Option<(i32, i32, i32)>,
        /// Optional position to set when enabling: (x, y)
        position: Option<(i32, i32)>,
    },
    /// Switch a monitor to a specific mode
    SwitchMode {
        /// Name of the monitor to configure
        name: String,
        /// Desired width in pixels
        width: i32,
        /// Desired height in pixels
        height: i32,
        /// Desired refresh rate in Hz
        refresh_rate: i32,
    },
    /// Set a monitor's scale factor
    SetScale {
        /// Name of the monitor to configure (e.g., "DP-1")
        name: String,
        /// Scale factor to apply (must be > 0, e.g., 1.0, 1.5, 2.0)
        scale: f64,
    },
    /// Set a monitor's transform (rotation/flip)
    SetTransform {
        /// Name of the monitor to configure (e.g., "DP-1")
        name: String,
        /// The desired transform
        transform: WlTransform,
    },
    /// Set a monitor's position in the global coordinate space
    SetPosition {
        /// Name of the monitor to configure (e.g., "DP-1")
        name: String,
        /// X coordinate in the global coordinate space
        x: i32,
        /// Y coordinate in the global coordinate space
        y: i32,
    },
}

impl WlMonitorManager {
    pub(super) fn handle_action(
        &mut self,
        action: WlMonitorAction,
        eq: &mut EventQueue<Self>,
    ) -> Result<(), WlMonitorManagerError> {
        let serial = self.serial.ok_or_else(|| {
            WlMonitorManagerError::EventQueueError("no serial available".into())
        })?;
        let manager = self.zwlr_manager.as_ref().ok_or_else(|| {
            WlMonitorManagerError::EventQueueError(
                "no manager available".into(),
            )
        })?;

        let qh = eq.handle();
        let config = manager.create_configuration(serial, &qh, ());

        match action {
            WlMonitorAction::Toggle {
                ref name,
                mode,
                position,
            } => {
                self.configure_toggle(&config, name, &qh, mode, position);
            }
            WlMonitorAction::SwitchMode {
                ref name,
                width,
                height,
                refresh_rate,
            } => {
                self.configure_switch_mode(
                    &config,
                    name,
                    width,
                    height,
                    refresh_rate,
                    &qh,
                );
            }
            WlMonitorAction::SetScale { ref name, scale } => {
                self.configure_set_scale(&config, name, scale, &qh);
            }
            WlMonitorAction::SetTransform {
                ref name,
                transform,
            } => {
                self.configure_set_transform(&config, name, transform, &qh);
            }
            WlMonitorAction::SetPosition { ref name, x, y } => {
                self.configure_set_position(&config, name, x, y, &qh);
            }
        }

        config.apply();
        match self.wait_for_result(eq) {
            Ok(()) => {}
            Err(e) => {
                let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                    action: ActionKind::ConfigApply,
                    reason: format!("{:?}", e),
                });
            }
        }
        config.destroy();

        Ok(())
    }

    fn configure_toggle(
        &mut self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        qh: &QueueHandle<Self>,
        mode: Option<(i32, i32, i32)>,
        position: Option<(i32, i32)>,
    ) {
        let target_enabled = self
            .monitors
            .values()
            .find(|m| m.name == name)
            .map(|m| m.enabled)
            .unwrap_or(false);

        // Save last_mode before the main loop so the mutable borrow is scoped separately
        if target_enabled {
            if let Some(monitor) =
                self.monitors.values_mut().find(|m| m.name == name)
            {
                if let Some(current_mode) = &monitor.current_mode {
                    monitor.last_mode = Some(current_mode.id());
                }
            }
        }

        for monitor in self.monitors.values() {
            if monitor.name != name {
                Self::preserve_head(config, monitor, qh);
                continue;
            }

            if target_enabled {
                config.disable_head(&monitor.head);
                continue;
            }

            let resolved_mode =
                if let Some((width, height, refresh_rate)) = mode {
                    monitor.modes.iter().find(|m| {
                        m.resolution.width == width
                            && m.resolution.height == height
                            && m.refresh_rate == refresh_rate
                    })
                } else if let Some(last_mode) = &monitor.last_mode {
                    monitor.modes.iter().find(|m| m.mode_id == *last_mode)
                } else {
                    None
                };

            let resolved_mode = resolved_mode
                .or_else(|| monitor.modes.iter().find(|m| m.preferred))
                .or_else(|| monitor.modes.first());

            if let Some(target_mode) = resolved_mode {
                let head = config.enable_head(&monitor.head, qh, ());
                head.set_mode(&target_mode.proxy);
                let (pos_x, pos_y) = if let Some((x, y)) = position {
                    (x, y)
                } else {
                    (monitor.position.x, monitor.position.y)
                };
                head.set_position(pos_x, pos_y);
                head.set_transform(monitor.transform.to_wayland());
                head.set_scale(monitor.scale);
            } else {
                let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                    action: ActionKind::Toggle,
                    reason: format!(
                        "No valid mode available for monitor '{}'",
                        name
                    ),
                });
            }
        }
    }

    fn configure_switch_mode(
        &mut self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        width: i32,
        height: i32,
        refresh_rate: i32,
        qh: &QueueHandle<Self>,
    ) {
        for monitor in self.monitors.values() {
            if monitor.name != name {
                Self::preserve_head(config, monitor, qh);
                continue;
            }

            if let Some(mode) = monitor.modes.iter().find(|m| {
                m.resolution.width == width
                    && m.resolution.height == height
                    && m.refresh_rate == refresh_rate
            }) {
                let config_head = config.enable_head(&monitor.head, qh, ());
                config_head.set_mode(&mode.proxy);
                config_head
                    .set_position(monitor.position.x, monitor.position.y);
                config_head.set_transform(monitor.transform.to_wayland());
                config_head.set_scale(monitor.scale);
            } else {
                Self::preserve_head(config, monitor, qh);
                let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                    action: ActionKind::SwitchMode,
                    reason: format!(
                        "No matching mode {}x{}@{}Hz for monitor '{}'",
                        width, height, refresh_rate, name
                    ),
                });
            }
        }
    }

    fn configure_set_scale(
        &self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        scale: f64,
        qh: &QueueHandle<Self>,
    ) {
        if !scale.is_finite() || scale <= 0.0 {
            let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                action: ActionKind::SetScale,
                reason: format!(
                    "Invalid scale value '{}': must be finite and > 0",
                    scale
                ),
            });
            for monitor in self.monitors.values() {
                Self::preserve_head(config, monitor, qh);
            }
            return;
        }

        for monitor in self.monitors.values() {
            if monitor.name != name {
                Self::preserve_head(config, monitor, qh);
                continue;
            }

            if !monitor.enabled {
                Self::preserve_head(config, monitor, qh);
                let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                    action: ActionKind::SetScale,
                    reason: format!(
                        "Monitor '{}' is disabled, cannot set scale",
                        name
                    ),
                });
                continue;
            }

            let config_head = config.enable_head(&monitor.head, qh, ());
            if let Some(ref current_mode) = monitor.current_mode {
                config_head.set_mode(current_mode);
            }
            config_head.set_position(monitor.position.x, monitor.position.y);
            config_head.set_transform(monitor.transform.to_wayland());
            config_head.set_scale(scale);
        }
    }

    fn configure_set_transform(
        &self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        transform: WlTransform,
        qh: &QueueHandle<Self>,
    ) {
        for monitor in self.monitors.values() {
            if monitor.name != name {
                Self::preserve_head(config, monitor, qh);
                continue;
            }

            if !monitor.enabled {
                Self::preserve_head(config, monitor, qh);
                let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                    action: ActionKind::SetTransform,
                    reason: format!(
                        "Monitor '{}' is disabled, cannot set transform",
                        name
                    ),
                });
                continue;
            }

            let config_head = config.enable_head(&monitor.head, qh, ());
            if let Some(ref current_mode) = monitor.current_mode {
                config_head.set_mode(current_mode);
            }
            config_head.set_position(monitor.position.x, monitor.position.y);
            config_head.set_transform(transform.to_wayland());
            config_head.set_scale(monitor.scale);
        }
    }

    fn configure_set_position(
        &self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        x: i32,
        y: i32,
        qh: &QueueHandle<Self>,
    ) {
        for monitor in self.monitors.values() {
            if monitor.name != name {
                Self::preserve_head(config, monitor, qh);
                continue;
            }

            if !monitor.enabled {
                Self::preserve_head(config, monitor, qh);
                let _ = self.emitter.send(WlMonitorEvent::ActionFailed {
                    action: ActionKind::SetPosition,
                    reason: format!(
                        "Monitor '{}' is disabled, cannot set position",
                        name
                    ),
                });
                continue;
            }

            let config_head = config.enable_head(&monitor.head, qh, ());
            if let Some(ref current_mode) = monitor.current_mode {
                config_head.set_mode(current_mode);
            }
            config_head.set_position(x, y);
            config_head.set_transform(monitor.transform.to_wayland());
            config_head.set_scale(monitor.scale);
        }
    }

    fn preserve_head(
        config: &ZwlrOutputConfigurationV1,
        monitor: &WlMonitor,
        qh: &QueueHandle<Self>,
    ) {
        if monitor.enabled {
            let config_head = config.enable_head(&monitor.head, qh, ());
            if let Some(ref current_mode) = monitor.current_mode {
                config_head.set_mode(current_mode);
            }
            config_head.set_position(monitor.position.x, monitor.position.y);
            config_head.set_transform(monitor.transform.to_wayland());
            config_head.set_scale(monitor.scale);
        } else {
            config.disable_head(&monitor.head);
        }
    }
}
