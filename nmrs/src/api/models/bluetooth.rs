use std::fmt::{Display, Formatter};

use crate::util::validation::validate_bluetooth_address;

use super::device::DeviceState;
use super::error::ConnectionError;

/// Bluetooth network role.
///
/// Specifies the role of the Bluetooth device in the network connection.
///
/// # Stability
///
/// This enum is marked as `#[non_exhaustive]` so as to assume that new Bluetooth roles may be
/// added in future versions. When pattern matching, always include a wildcard arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BluetoothNetworkRole {
    PanU, // Personal Area Network User
    Dun,  // Dial-Up Networking
}

/// Bluetooth device identity information.
///
/// Relevant info for Bluetooth devices managed by NetworkManager.
///
/// # Example
///```rust
/// use nmrs::models::{BluetoothIdentity, BluetoothNetworkRole};
///
/// let bt_settings = BluetoothIdentity::new(
///    "00:1A:7D:DA:71:13".into(),
///    BluetoothNetworkRole::Dun,
/// ).unwrap();
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BluetoothIdentity {
    /// MAC address of Bluetooth device
    pub bdaddr: String,
    /// Bluetooth device type (DUN or PANU)
    pub bt_device_type: BluetoothNetworkRole,
    /// Optional Bluetooth adapter name (e.g., "hci1"). When `None`, the
    /// adapter owning `bdaddr` is looked up through BlueZ.
    pub adapter: Option<String>,
}

impl BluetoothIdentity {
    /// Creates a new `BluetoothIdentity`.
    ///
    /// # Arguments
    ///
    /// * `bdaddr` - Bluetooth MAC address (e.g., "00:1A:7D:DA:71:13")
    /// * `bt_device_type` - Bluetooth network role (PanU or Dun)
    ///
    /// # Errors
    ///
    /// Returns a `ConnectionError` if the provided `bdaddr` is not a
    /// valid Bluetooth MAC address format.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nmrs::models::{BluetoothIdentity, BluetoothNetworkRole};
    ///
    /// let identity = BluetoothIdentity::new(
    ///     "00:1A:7D:DA:71:13".into(),
    ///     BluetoothNetworkRole::PanU,
    /// ).unwrap();
    /// ```
    pub fn new(
        bdaddr: String,
        bt_device_type: BluetoothNetworkRole,
    ) -> Result<Self, ConnectionError> {
        validate_bluetooth_address(&bdaddr)?;
        Ok(Self {
            bdaddr,
            bt_device_type,
            adapter: None,
        })
    }

    /// Creates a new `BluetoothIdentity` with a specific adapter.
    ///
    /// Pinning the adapter skips the BlueZ lookup that [`Self::new`] relies on,
    /// so the device is addressed on `adapter` even if BlueZ reports it
    /// elsewhere.
    ///
    /// # Arguments
    ///
    /// * `bdaddr` - Bluetooth MAC address (e.g., "00:1A:7D:DA:71:13")
    /// * `bt_device_type` - Bluetooth network role (PanU or Dun)
    /// * `adapter` - Bluetooth adapter name (e.g., "hci1")
    ///
    /// # Errors
    ///
    /// Returns a `ConnectionError` if the provided `bdaddr` is not a
    /// valid Bluetooth MAC address format.
    pub fn with_adapter(
        bdaddr: String,
        bt_device_type: BluetoothNetworkRole,
        adapter: String,
    ) -> Result<Self, ConnectionError> {
        validate_bluetooth_address(&bdaddr)?;
        Ok(Self {
            bdaddr,
            bt_device_type,
            adapter: Some(adapter),
        })
    }
}

/// Bluetooth device with friendly name from BlueZ.
///
/// Contains information about a Bluetooth device managed by NetworkManager,
/// proxying data from BlueZ.
///
/// This is a specialized struct for Bluetooth devices, separate from the
/// general `Device` struct.
///
/// # Example
///
/// # Example
///
/// ```rust
/// use nmrs::models::{BluetoothDevice, BluetoothNetworkRole, DeviceState};
///
/// let role = BluetoothNetworkRole::PanU as u32;
/// let device = BluetoothDevice::new(
///     "00:1A:7D:DA:71:13".into(),
///     Some("My Phone".into()),
///     Some("Phone".into()),
///     role,
///     DeviceState::Activated,
/// );
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    /// Bluetooth MAC address
    pub bdaddr: String,
    /// Friendly device name from BlueZ
    pub name: Option<String>,
    /// Device alias from BlueZ
    pub alias: Option<String>,
    /// Bluetooth device type (DUN or PANU)
    pub bt_caps: u32,
    /// Current device state
    pub state: DeviceState,
}

impl BluetoothDevice {
    /// Creates a new `BluetoothDevice`.
    ///
    /// # Arguments
    ///
    /// * `bdaddr` - Bluetooth MAC address
    /// * `name` - Friendly device name from BlueZ
    /// * `alias` - Device alias from BlueZ
    /// * `bt_caps` - Bluetooth device capabilities/type
    /// * `state` - Current device state
    ///
    /// # Example
    ///
    /// ```rust
    /// use nmrs::models::{BluetoothDevice, BluetoothNetworkRole, DeviceState};
    ///
    /// let role = BluetoothNetworkRole::PanU as u32;
    /// let device = BluetoothDevice::new(
    ///     "00:1A:7D:DA:71:13".into(),
    ///     Some("My Phone".into()),
    ///     Some("Phone".into()),
    ///     role,
    ///     DeviceState::Activated,
    /// );
    /// ```
    #[must_use]
    pub fn new(
        bdaddr: String,
        name: Option<String>,
        alias: Option<String>,
        bt_caps: u32,
        state: DeviceState,
    ) -> Self {
        Self {
            bdaddr,
            name,
            alias,
            bt_caps,
            state,
        }
    }
}

impl Display for BluetoothDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let role = BluetoothNetworkRole::from(self.bt_caps);
        write!(
            f,
            "{} ({}) [{}]",
            self.alias.as_deref().unwrap_or("unknown"),
            role,
            self.bdaddr
        )
    }
}

impl Display for BluetoothNetworkRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BluetoothNetworkRole::Dun => write!(f, "DUN"),
            BluetoothNetworkRole::PanU => write!(f, "PANU"),
        }
    }
}

impl From<u32> for BluetoothNetworkRole {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::PanU,
            1 => Self::Dun,
            _ => Self::PanU,
        }
    }
}
