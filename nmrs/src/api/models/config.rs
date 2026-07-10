use std::time::Duration;

/// Timeout configuration for NetworkManager operations.
///
/// Controls how long NetworkManager will wait for various network operations
/// to complete before timing out. This allows customization for different
/// network environments (slow networks, enterprise auth, etc.).
///
/// # Examples
///
/// ```rust
/// use nmrs::TimeoutConfig;
/// use std::time::Duration;
///
/// // Use default timeouts (30s connect, 10s disconnect)
/// let config = TimeoutConfig::default();
///
/// // Custom timeouts for slow networks
/// let config = TimeoutConfig::new()
///     .with_connection_timeout(Duration::from_secs(60))
///     .with_disconnect_timeout(Duration::from_secs(20));
///
/// // Quick timeouts for fast networks
/// let config = TimeoutConfig::new()
///     .with_connection_timeout(Duration::from_secs(15))
///     .with_disconnect_timeout(Duration::from_secs(5));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct TimeoutConfig {
    /// Timeout for connection activation (default: 30 seconds)
    pub connection_timeout: Duration,
    /// Timeout for device disconnection (default: 10 seconds)
    pub disconnect_timeout: Duration,
}

impl Default for TimeoutConfig {
    /// Returns the default timeout configuration.
    ///
    /// Defaults:
    /// - `connection_timeout`: 30 seconds
    /// - `disconnect_timeout`: 10 seconds
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(30),
            disconnect_timeout: Duration::from_secs(10),
        }
    }
}

impl TimeoutConfig {
    /// Creates a new `TimeoutConfig` with default values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::TimeoutConfig;
    ///
    /// let config = TimeoutConfig::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the connection activation timeout.
    ///
    /// This controls how long to wait for a network connection to activate
    /// before giving up. Increase this for slow networks or enterprise
    /// authentication that may take longer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::TimeoutConfig;
    /// use std::time::Duration;
    ///
    /// let config = TimeoutConfig::new()
    ///     .with_connection_timeout(Duration::from_secs(60));
    /// ```
    #[must_use]
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Sets the disconnection timeout.
    ///
    /// This controls how long to wait for a device to disconnect before
    /// giving up.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::TimeoutConfig;
    /// use std::time::Duration;
    ///
    /// let config = TimeoutConfig::new()
    ///     .with_disconnect_timeout(Duration::from_secs(20));
    /// ```
    #[must_use]
    pub fn with_disconnect_timeout(mut self, timeout: Duration) -> Self {
        self.disconnect_timeout = timeout;
        self
    }
}

/// Connection options for saved NetworkManager connections.
///
/// Controls how NetworkManager handles saved connection profiles,
/// including automatic connection behavior.
///
/// # Examples
///
/// ```rust
/// use nmrs::ConnectionOptions;
///
/// // Basic auto-connect (using defaults)
/// let opts = ConnectionOptions::default();
///
/// // High-priority connection with retry limit
/// let opts_priority = ConnectionOptions::new(true)
///     .with_priority(10)  // Higher = more preferred
///     .with_retries(3);   // Retry up to 3 times
///
/// // Manual connection only
/// let opts_manual = ConnectionOptions::new(false);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    /// Whether to automatically connect when available.
    pub autoconnect: bool,

    /// Priority for auto-connection (higher = more preferred).
    ///
    /// NetworkManager uses this value to determine which network to prefer when
    /// multiple configured networks are available at the same time.
    ///
    /// - **Higher values** have higher priority.
    /// - **Default value** is `0` (if set to `None`).
    /// - **Negative values** are allowed and indicate lower priority than default.
    pub autoconnect_priority: Option<i32>,

    /// Maximum number of auto-connect retry attempts.
    ///
    /// Configures how many times NetworkManager will attempt to automatically
    /// reconnect if activation fails.
    ///
    /// - `Some(0)`: try indefinitely.
    /// - `Some(n)`: retry up to `n` times.
    /// - `None`: uses NetworkManager's global default configuration (4 attempts).
    pub autoconnect_retries: Option<i32>,
}

impl Default for ConnectionOptions {
    /// Returns the default connection options.
    ///
    /// Defaults:
    /// - `autoconnect`: `true`
    /// - `autoconnect_priority`: `None` (uses NetworkManager's default of 0)
    /// - `autoconnect_retries`: `None` (unlimited retries)
    fn default() -> Self {
        Self {
            autoconnect: true,
            autoconnect_priority: None,
            autoconnect_retries: None,
        }
    }
}

impl ConnectionOptions {
    /// Creates new `ConnectionOptions` with the specified autoconnect setting.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::ConnectionOptions;
    ///
    /// let opts = ConnectionOptions::new(true);
    /// ```
    #[must_use]
    pub fn new(autoconnect: bool) -> Self {
        Self {
            autoconnect,
            autoconnect_priority: None,
            autoconnect_retries: None,
        }
    }

    /// Sets the auto-connection priority.
    #[must_use]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.autoconnect_priority = Some(priority);
        self
    }

    /// Sets the maximum number of auto-connect retry attempts.
    #[must_use]
    pub fn with_retries(mut self, retries: i32) -> Self {
        self.autoconnect_retries = Some(retries);
        self
    }
}
