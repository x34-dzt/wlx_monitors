//! Wayland output manager using wlr-output-management protocol
//!
//! This crate provides a simple interface to detect, monitor, and control
//! display outputs on Wayland compositors that support the
//! `zwlr_output_manager_v1` protocol (wlroots-based compositors).
//!
//! # Example
//!
//! ```no_run
//! use wlx_monitors::{WlMonitorManager, WlMonitorEvent, WlMonitorAction};
//! use std::sync::mpsc::sync_channel;
//!
//! let (tx, rx) = sync_channel(10);
//! let (action_tx, action_rx) = sync_channel(10);
//!
//! let (manager, event_queue) = WlMonitorManager::new_connection(tx, action_rx).unwrap();
//!
//! // Run the manager in a separate thread or async context
//! // to receive monitor events and send actions
//! ```

mod state;
mod wl_monitor;

pub use state::{
    ActionKind, WlMonitorAction, WlMonitorEvent, WlMonitorManager,
    WlMonitorManagerError,
};
pub use wl_monitor::{
    WlMonitor, WlMonitorMode, WlPosition, WlResolution, WlTransform,
};
