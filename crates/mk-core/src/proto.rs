//! Control-channel protocol between `mk-app` and the driver's hub device.
//!
//! The channel only carries configuration and status — never key events.

/// Device interface GUID exposed by the hub device: `{850ccc0b-e460-4555-bb3f-474dec769d11}`.
pub const INTERFACE_GUID: (u32, u16, u16, [u8; 8]) = (
    0x850c_cc0b,
    0xe460,
    0x4555,
    [0xbb, 0x3f, 0x47, 0x4d, 0xec, 0x76, 0x9d, 0x11],
);

/// Custom device type (0x8000+ range is reserved for vendors).
pub const DEVICE_TYPE: u32 = 0x8A11;

const METHOD_BUFFERED: u32 = 0;
const FILE_READ_ACCESS: u32 = 0x0001;
const FILE_WRITE_ACCESS: u32 = 0x0002;

/// Equivalent of the Windows `CTL_CODE` macro.
pub const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

/// Out: [`Status`].
pub const IOCTL_GET_STATUS: u32 = ctl_code(DEVICE_TYPE, 0x800, METHOD_BUFFERED, FILE_READ_ACCESS);
/// Out: 64-byte config wire buffer ([`crate::Config::to_wire`]).
pub const IOCTL_GET_CONFIG: u32 = ctl_code(DEVICE_TYPE, 0x801, METHOD_BUFFERED, FILE_READ_ACCESS);
/// In: 64-byte config wire buffer. Applied live and persisted in the registry.
pub const IOCTL_SET_CONFIG: u32 = ctl_code(
    DEVICE_TYPE,
    0x802,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);
/// In: [`Status::generation`] last seen (u32). Completes (out: [`Status`]) when the status changes.
pub const IOCTL_WAIT_STATUS: u32 = ctl_code(DEVICE_TYPE, 0x803, METHOD_BUFFERED, FILE_READ_ACCESS);

pub const PROTOCOL_VERSION: u16 = 1;
pub const STATUS_SIZE: usize = 16;

/// Driver status snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub protocol_version: u16,
    /// Number of Apple keyboards currently filtered.
    pub keyboards: u8,
    /// Current Fn mode (0 = media first, 1 = F-keys first); may change via Fn double-tap.
    pub fn_mode: u8,
    /// True when the virtual HID device is up.
    pub virtual_device_ready: bool,
    /// Product ID of the most recently attached keyboard (0 if none).
    pub last_pid: u16,
    /// Incremented on every status change.
    pub generation: u32,
}

impl Status {
    pub fn to_bytes(&self) -> [u8; STATUS_SIZE] {
        let mut b = [0u8; STATUS_SIZE];
        b[0..2].copy_from_slice(&self.protocol_version.to_le_bytes());
        b[2] = self.keyboards;
        b[3] = self.fn_mode;
        b[4] = self.virtual_device_ready as u8;
        b[6..8].copy_from_slice(&self.last_pid.to_le_bytes());
        b[8..12].copy_from_slice(&self.generation.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < STATUS_SIZE {
            return None;
        }
        Some(Status {
            protocol_version: u16::from_le_bytes([b[0], b[1]]),
            keyboards: b[2],
            fn_mode: b[3],
            virtual_device_ready: b[4] != 0,
            last_pid: u16::from_le_bytes([b[6], b[7]]),
            generation: u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctl_codes_match_windows_macro() {
        // CTL_CODE(0x8A11, 0x800, METHOD_BUFFERED, FILE_READ_ACCESS) computed by hand.
        assert_eq!(IOCTL_GET_STATUS, 0x8A11_6000);
        assert_eq!(IOCTL_SET_CONFIG, 0x8A11_E008);
    }

    #[test]
    fn status_roundtrip() {
        let s = Status {
            protocol_version: PROTOCOL_VERSION,
            keyboards: 1,
            fn_mode: 1,
            virtual_device_ready: true,
            last_pid: 0x029F,
            generation: 42,
        };
        assert_eq!(Status::from_bytes(&s.to_bytes()), Some(s));
    }
}
