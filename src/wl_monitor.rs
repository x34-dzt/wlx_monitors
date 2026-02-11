use wayland_client::{
    backend::ObjectId,
    protocol::wl_output::Transform,
    WEnum,
};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_head_v1::ZwlrOutputHeadV1,
    zwlr_output_mode_v1::ZwlrOutputModeV1,
};

/// Monitor transform (rotation/flip)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WlTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl WlTransform {
    pub(crate) fn from_wayland(t: WEnum<Transform>) -> Self {
        match t {
            WEnum::Value(Transform::Normal) => Self::Normal,
            WEnum::Value(Transform::_90) => Self::Rotate90,
            WEnum::Value(Transform::_180) => Self::Rotate180,
            WEnum::Value(Transform::_270) => Self::Rotate270,
            WEnum::Value(Transform::Flipped) => Self::Flipped,
            WEnum::Value(Transform::Flipped90) => Self::Flipped90,
            WEnum::Value(Transform::Flipped180) => Self::Flipped180,
            WEnum::Value(Transform::Flipped270) => Self::Flipped270,
            _ => Self::Normal,
        }
    }

    pub(crate) fn to_wayland(self) -> Transform {
        match self {
            Self::Normal => Transform::Normal,
            Self::Rotate90 => Transform::_90,
            Self::Rotate180 => Transform::_180,
            Self::Rotate270 => Transform::_270,
            Self::Flipped => Transform::Flipped,
            Self::Flipped90 => Transform::Flipped90,
            Self::Flipped180 => Transform::Flipped180,
            Self::Flipped270 => Transform::Flipped270,
        }
    }
}

/// Represents the resolution of a monitor mode
#[derive(Default, Clone, Debug)]
pub struct WlResolution {
    /// Height in pixels
    pub height: i32,
    /// Width in pixels
    pub width: i32,
}

/// Represents the position of a monitor in the global coordinate space
#[derive(Default, Clone, Debug)]
pub struct WlPosition {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
}

/// Represents a display mode (resolution + refresh rate) for a monitor
#[derive(Clone)]
pub struct WlMonitorMode {
    /// Internal Wayland object ID for this mode
    pub mode_id: ObjectId,
    /// Internal Wayland object ID for the monitor head this mode belongs to
    pub head_id: ObjectId,
    /// Refresh rate in Hz
    pub refresh_rate: i32,
    /// Screen resolution
    pub resolution: WlResolution,
    /// Whether this is the preferred mode for the monitor
    pub preferred: bool,
    /// Whether this is the currently active mode
    pub is_current: bool,
    /// Internal Wayland proxy object for this mode
    pub proxy: ZwlrOutputModeV1,
}

impl std::fmt::Debug for WlMonitorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WlMonitorMode")
            .field("mode_id", &self.mode_id)
            .field("head_id", &self.head_id)
            .field("refresh_rate", &self.refresh_rate)
            .field("resolution", &self.resolution)
            .field("preferred", &self.preferred)
            .field("is_current", &self.is_current)
            .finish_non_exhaustive()
    }
}

/// Represents a connected monitor/display
#[derive(Clone)]
pub struct WlMonitor {
    /// Internal Wayland object ID for the monitor head
    pub head_id: ObjectId,
    /// Monitor name (e.g., "DP-1", "HDMI-A-1")
    pub name: String,
    /// Human-readable description of the monitor
    pub description: String,
    /// Manufacturer name
    pub make: String,
    /// Model name
    pub model: String,
    /// Serial number
    pub serial_number: String,
    /// List of available display modes
    pub modes: Vec<WlMonitorMode>,
    /// Current resolution
    pub resolution: WlResolution,
    /// Current position in the global coordinate space
    pub position: WlPosition,
    /// Current scale factor (e.g., 1.0, 1.5, 2.0)
    pub scale: f64,
    /// Whether the monitor is currently enabled
    pub enabled: bool,
    /// Currently active mode (if any)
    pub current_mode: Option<ZwlrOutputModeV1>,
    /// Current transformation (normal, rotated, flipped, etc.)
    pub transform: WlTransform,
    /// Internal Wayland head proxy object
    pub head: ZwlrOutputHeadV1,
    /// Internal flag indicating if the monitor state has changed
    pub changed: bool,
    /// Stores the mode ID before the monitor was disabled
    pub last_mode: Option<ObjectId>,
}

impl std::fmt::Debug for WlMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WlMonitor")
            .field("head_id", &self.head_id)
            .field("name", &self.name)
            .field("description", &self.description)
            .field("make", &self.make)
            .field("model", &self.model)
            .field("serial_number", &self.serial_number)
            .field("modes", &self.modes)
            .field("resolution", &self.resolution)
            .field("position", &self.position)
            .field("scale", &self.scale)
            .field("enabled", &self.enabled)
            .field("transform", &self.transform)
            .field("changed", &self.changed)
            .field("last_mode", &self.last_mode)
            .finish_non_exhaustive()
    }
}
