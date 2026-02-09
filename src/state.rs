use std::{
    collections::HashMap,
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender},
    },
};

use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
    backend::ObjectId,
    protocol::{wl_output::Transform, wl_registry},
};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_configuration_head_v1::{self, ZwlrOutputConfigurationHeadV1},
    zwlr_output_configuration_v1::{self, ZwlrOutputConfigurationV1},
    zwlr_output_head_v1::{self, ZwlrOutputHeadV1},
    zwlr_output_manager_v1::{self, ZwlrOutputManagerV1},
    zwlr_output_mode_v1::{self, ZwlrOutputModeV1},
};

use crate::wl_monitor::{WlMonitor, WlMonitorMode, WlPosition, WlResolution};

/// Events emitted by the Wayland monitor manager
#[derive(Debug)]
pub enum WlMonitorEvent {
    /// Sent once when the initial state is received, containing all connected monitors
    InitialState(Vec<WlMonitor>),
    /// Sent when a monitor's properties have changed
    Changed(Box<WlMonitor>),
    /// Sent when a monitor is disconnected
    Removed { id: ObjectId, name: String },
}

/// Actions that can be sent to the monitor manager to control monitors
pub enum WlMonitorAction {
    /// Toggle a monitor on/off by name
    Toggle {
        /// Name of the monitor to toggle (e.g., "DP-1")
        name: String,
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
}

#[derive(Debug, PartialEq)]
enum ConfigResult {
    Idle,
    Succeeded,
    Failed,
    Cancelled,
}

/// Manages Wayland monitor/output state and communication
///
/// This struct handles the connection to the Wayland display and provides
/// an interface to receive monitor events and send control actions.
pub struct WlMonitorManager {
    _conn: Connection,
    emitter: SyncSender<WlMonitorEvent>,
    monitors: HashMap<ObjectId, WlMonitor>,
    mode_monitor: HashMap<ObjectId, ObjectId>,
    controller: Receiver<WlMonitorAction>,
    zwlr_manager: Option<ZwlrOutputManagerV1>,
    serial: Option<u32>,
    initialized: bool,
    config_result: ConfigResult,
}

/// Errors that can occur when using the monitor manager
#[derive(Debug)]
pub enum WlMonitorManagerError {
    /// Failed to establish Wayland connection
    ConnectionError(String),
    /// Error in the Wayland event queue
    EventQueueError(String),
}

impl WlMonitorManager {
    /// Create a new Wayland connection and monitor manager
    ///
    /// Returns the manager and an event queue that must be dispatched to process events.
    ///
    /// # Arguments
    ///
    /// * `emitter` - Channel sender for receiving monitor events
    /// * `controller` - Channel receiver for sending control actions
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError` if unable to connect to the Wayland display.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use wlx_monitors::{WlMonitorManager, WlMonitorEvent, WlMonitorAction};
    /// use std::sync::mpsc::sync_channel;
    ///
    /// let (tx, rx) = sync_channel(10);
    /// let (action_tx, action_rx) = sync_channel(10);
    ///
    /// let (manager, event_queue) = WlMonitorManager::new_connection(tx, action_rx).unwrap();
    /// ```
    pub fn new_connection(
        emitter: SyncSender<WlMonitorEvent>,
        controller: Receiver<WlMonitorAction>,
    ) -> Result<(Self, EventQueue<Self>), WlMonitorManagerError> {
        let conn = Connection::connect_to_env().map_err(|e| {
            WlMonitorManagerError::ConnectionError(e.to_string())
        })?;

        let display_object = conn.display();
        let event_queue: EventQueue<WlMonitorManager> = conn.new_event_queue();
        let queue_handler = event_queue.handle();
        display_object.get_registry(&queue_handler, ());

        let state = WlMonitorManager {
            _conn: conn,
            emitter,
            monitors: HashMap::new(),
            mode_monitor: HashMap::new(),
            controller,
            zwlr_manager: None,
            serial: None,
            initialized: false,
            config_result: ConfigResult::Idle,
        };

        Ok((state, event_queue))
    }

    /// Run the monitor manager event loop
    ///
    /// This will block and process events indefinitely, sending monitor events
    /// through the emitter channel and receiving actions from the controller channel.
    ///
    /// # Errors
    ///
    /// Returns `EventQueueError` if there's an error in the Wayland event queue.
    ///
    /// # Note
    ///
    /// This function runs indefinitely until an error occurs. Run it in a separate thread.
    pub fn run(
        mut self,
        mut eq: EventQueue<Self>,
    ) -> Result<(), WlMonitorManagerError> {
        loop {
            eq.flush().map_err(|e| {
                WlMonitorManagerError::EventQueueError(e.to_string())
            })?;

            let guard = eq.prepare_read().unwrap();
            let fd = guard.connection_fd();
            let mut poll_fd = [rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN,
            )];
            let timeout = rustix::time::Timespec {
                tv_sec: 0,
                tv_nsec: 50_000_000,
            };
            let _ = rustix::event::poll(&mut poll_fd, Some(&timeout));
            let _ = guard.read();
            eq.dispatch_pending(&mut self).map_err(|e| {
                WlMonitorManagerError::EventQueueError(e.to_string())
            })?;
            self.flush_changed();

            if let Ok(action) = self.controller.try_recv() {
                self.handle_action(action, &mut eq)?;
            }
        }
    }

    fn flush_changed(&mut self) {
        if !self.initialized {
            return;
        }
        for monitor in self.monitors.values_mut() {
            if monitor.changed {
                monitor.changed = false;
                let _ = self
                    .emitter
                    .send(WlMonitorEvent::Changed(Box::new(monitor.clone())));
            }
        }
    }

    fn handle_action(
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
            WlMonitorAction::Toggle { ref name } => {
                self.configure_toggle(&config, name, &qh);
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
        }

        config.apply();
        self.wait_for_result(eq)?;
        config.destroy();

        Ok(())
    }

    fn configure_toggle(
        &self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        qh: &QueueHandle<Self>,
    ) {
        let target_enabled = self
            .monitors
            .values()
            .find(|m| m.name == name)
            .map(|m| m.enabled)
            .unwrap_or(false);

        for monitor in self.monitors.values() {
            if monitor.name == name {
                if target_enabled {
                    config.disable_head(&monitor.head);
                } else {
                    let config_head = config.enable_head(&monitor.head, qh, ());
                    if let Some(mode) = monitor
                        .modes
                        .iter()
                        .find(|m| m.preferred)
                        .or_else(|| monitor.modes.first())
                    {
                        config_head.set_mode(&mode.proxy);
                    }
                }
            } else {
                self.preserve_head(config, monitor, qh);
            }
        }
    }

    fn configure_switch_mode(
        &self,
        config: &ZwlrOutputConfigurationV1,
        name: &str,
        width: i32,
        height: i32,
        refresh_rate: i32,
        qh: &QueueHandle<Self>,
    ) {
        for monitor in self.monitors.values() {
            if monitor.name == name {
                let config_head = config.enable_head(&monitor.head, qh, ());
                if let Some(mode) = monitor.modes.iter().find(|m| {
                    m.resolution.width == width
                        && m.resolution.height == height
                        && m.refresh_rate == refresh_rate
                }) {
                    config_head.set_mode(&mode.proxy);
                }
                if monitor.enabled {
                    config_head
                        .set_position(monitor.position.x, monitor.position.y);
                    if let WEnum::Value(t) = monitor.transform {
                        config_head.set_transform(t);
                    }
                    config_head.set_scale(monitor.scale);
                }
            } else {
                self.preserve_head(config, monitor, qh);
            }
        }
    }

    fn preserve_head(
        &self,
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
            if let WEnum::Value(t) = monitor.transform {
                config_head.set_transform(t);
            }
            config_head.set_scale(monitor.scale);
        } else {
            config.disable_head(&monitor.head);
        }
    }

    fn wait_for_result(
        &mut self,
        eq: &mut EventQueue<Self>,
    ) -> Result<(), WlMonitorManagerError> {
        self.config_result = ConfigResult::Idle;
        while self.config_result == ConfigResult::Idle {
            eq.blocking_dispatch(self).map_err(|e| {
                WlMonitorManagerError::EventQueueError(e.to_string())
            })?;
            self.flush_changed();
        }
        match self.config_result {
            ConfigResult::Succeeded => Ok(()),
            ConfigResult::Failed => {
                Err(WlMonitorManagerError::EventQueueError(
                    "compositor rejected the configuration".into(),
                ))
            }
            ConfigResult::Cancelled => {
                Err(WlMonitorManagerError::EventQueueError(
                    "configuration cancelled (serial outdated)".into(),
                ))
            }
            ConfigResult::Idle => unreachable!(),
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WlMonitorManager {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == ZwlrOutputManagerV1::interface().name
        {
            let bound = registry.bind::<ZwlrOutputManagerV1, _, _>(
                name,
                version,
                qh,
                (),
            );
            state.zwlr_manager = Some(bound);
        }
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for WlMonitorManager {
    fn event(
        state: &mut Self,
        _: &ZwlrOutputManagerV1,
        event: zwlr_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => {
                state.monitors.insert(
                    head.id(),
                    WlMonitor {
                        head_id: head.id(),
                        name: String::new(),
                        description: String::new(),
                        make: String::new(),
                        model: String::new(),
                        serial_number: String::new(),
                        modes: Vec::new(),
                        resolution: WlResolution::default(),
                        position: WlPosition::default(),
                        scale: 1.0,
                        enabled: false,
                        current_mode: None,
                        transform: WEnum::Value(Transform::Normal),
                        head,
                        changed: false,
                    },
                );
            }
            zwlr_output_manager_v1::Event::Done { serial } => {
                state.serial = Some(serial);
                if !state.initialized {
                    state.initialized = true;
                    let monitors = state.monitors.values().cloned().collect();
                    let _ = state
                        .emitter
                        .send(WlMonitorEvent::InitialState(monitors));
                }
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        if opcode == 0 {
            qh.make_data::<ZwlrOutputHeadV1, _>(())
        } else {
            unreachable!()
        }
    }
}

impl Dispatch<ZwlrOutputHeadV1, ()> for WlMonitorManager {
    fn event(
        state: &mut Self,
        head: &ZwlrOutputHeadV1,
        event: <ZwlrOutputHeadV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let head_id = head.id();

        if let zwlr_output_head_v1::Event::Finished = &event {
            if let Some(monitor) = state.monitors.remove(&head_id) {
                let _ = state.emitter.send(WlMonitorEvent::Removed {
                    id: monitor.head_id,
                    name: monitor.name,
                });
            }
            return;
        }

        let Some(monitor) = state.monitors.get_mut(&head_id) else {
            return;
        };

        if let zwlr_output_head_v1::Event::Mode { mode } = &event {
            state.mode_monitor.insert(mode.id(), head_id);
            monitor.modes.push(WlMonitorMode {
                mode_id: mode.id(),
                head_id: monitor.head_id.clone(),
                refresh_rate: 0,
                resolution: WlResolution::default(),
                preferred: false,
                is_current: false,
                proxy: mode.clone(),
            });
            return;
        }

        match event {
            zwlr_output_head_v1::Event::Name { name } => {
                monitor.name = name;
            }
            zwlr_output_head_v1::Event::Description { description } => {
                monitor.description = description;
            }
            zwlr_output_head_v1::Event::Make { make } => {
                monitor.make = make;
            }
            zwlr_output_head_v1::Event::Model { model } => {
                monitor.model = model;
            }
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                monitor.serial_number = serial_number;
            }
            zwlr_output_head_v1::Event::Enabled { enabled } => {
                monitor.enabled = enabled != 0;
            }
            zwlr_output_head_v1::Event::CurrentMode { mode } => {
                monitor.current_mode = Some(mode.clone());
                // Update is_current flag for all modes
                for m in &mut monitor.modes {
                    m.is_current = m.mode_id == mode.id();
                }
            }
            zwlr_output_head_v1::Event::Position { x, y } => {
                monitor.position = WlPosition { x, y };
            }
            zwlr_output_head_v1::Event::Scale { scale } => {
                monitor.scale = scale;
            }
            zwlr_output_head_v1::Event::Transform { transform } => {
                monitor.transform = transform;
            }
            _ => {}
        }

        if state.initialized {
            monitor.changed = true;
        }
    }

    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        if opcode == 3 {
            qh.make_data::<ZwlrOutputModeV1, _>(())
        } else {
            unreachable!()
        }
    }
}

impl Dispatch<ZwlrOutputModeV1, ()> for WlMonitorManager {
    fn event(
        state: &mut Self,
        mode_obj: &ZwlrOutputModeV1,
        event: <ZwlrOutputModeV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mode_id = mode_obj.id();
        let Some(monitor_id) = state.mode_monitor.get(&mode_id) else {
            return;
        };
        let Some(monitor) = state.monitors.get_mut(monitor_id) else {
            return;
        };
        let Some(mode) =
            monitor.modes.iter_mut().find(|m| m.mode_id == mode_id)
        else {
            return;
        };
        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => {
                mode.resolution = WlResolution { width, height };
            }
            zwlr_output_mode_v1::Event::Refresh { refresh } => {
                mode.refresh_rate = refresh / 1000;
            }
            zwlr_output_mode_v1::Event::Preferred => {
                mode.preferred = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputConfigurationV1, ()> for WlMonitorManager {
    fn event(
        state: &mut Self,
        _: &ZwlrOutputConfigurationV1,
        event: zwlr_output_configuration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_configuration_v1::Event::Succeeded => {
                state.config_result = ConfigResult::Succeeded;
            }
            zwlr_output_configuration_v1::Event::Failed => {
                state.config_result = ConfigResult::Failed;
            }
            zwlr_output_configuration_v1::Event::Cancelled => {
                state.config_result = ConfigResult::Cancelled;
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputConfigurationHeadV1, ()> for WlMonitorManager {
    fn event(
        _: &mut Self,
        _: &ZwlrOutputConfigurationHeadV1,
        _event: zwlr_output_configuration_head_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
