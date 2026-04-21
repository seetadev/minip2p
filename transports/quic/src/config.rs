/// Configuration for a QUIC transport node.
#[derive(Clone, Debug)]
pub struct QuicNodeConfig {
    pub(crate) local_cert_chain_path: Option<String>,
    pub(crate) local_priv_key_path: Option<String>,
    pub(crate) peer_verification: bool,
}

impl Default for QuicNodeConfig {
    fn default() -> Self {
        Self {
            local_cert_chain_path: None,
            local_priv_key_path: None,
            peer_verification: false,
        }
    }
}

impl QuicNodeConfig {
    /// Creates a default configuration (no TLS, no peer verification).
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience config for a development dialer (no TLS, no verification).
    pub fn dev_dialer() -> Self {
        Self::new().with_peer_verification(false)
    }

    /// Convenience config for a development listener with TLS.
    pub fn dev_listener_with_tls(cert: impl Into<String>, key: impl Into<String>) -> Self {
        Self::dev_dialer().with_local_tls_files(cert, key)
    }

    /// Sets the local TLS certificate chain and private key PEM file paths.
    pub fn with_local_tls_files(mut self, cert: impl Into<String>, key: impl Into<String>) -> Self {
        self.local_cert_chain_path = Some(cert.into());
        self.local_priv_key_path = Some(key.into());
        self
    }

    /// Enables or disables peer certificate verification.
    pub fn with_peer_verification(mut self, verify: bool) -> Self {
        self.peer_verification = verify;
        self
    }

    /// Returns `true` if TLS is configured (required for listening).
    pub fn can_listen(&self) -> bool {
        self.local_cert_chain_path.is_some() && self.local_priv_key_path.is_some()
    }

    /// Returns the TLS certificate chain PEM path, if configured.
    pub(crate) fn local_cert_chain_path(&self) -> Option<&str> {
        self.local_cert_chain_path.as_deref()
    }

    /// Returns the TLS private key PEM path, if configured.
    pub(crate) fn local_priv_key_path(&self) -> Option<&str> {
        self.local_priv_key_path.as_deref()
    }

    /// Returns whether peer certificate verification is enabled.
    pub(crate) fn peer_verification(&self) -> bool {
        self.peer_verification
    }
}
