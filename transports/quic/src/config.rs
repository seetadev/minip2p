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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dev_dialer() -> Self {
        Self::new().with_peer_verification(false)
    }

    pub fn dev_listener_with_tls(cert: impl Into<String>, key: impl Into<String>) -> Self {
        Self::dev_dialer().with_local_tls_files(cert, key)
    }

    pub fn with_local_tls_files(mut self, cert: impl Into<String>, key: impl Into<String>) -> Self {
        self.local_cert_chain_path = Some(cert.into());
        self.local_priv_key_path = Some(key.into());
        self
    }

    pub fn with_peer_verification(mut self, verify: bool) -> Self {
        self.peer_verification = verify;
        self
    }

    pub fn can_listen(&self) -> bool {
        self.local_cert_chain_path.is_some() && self.local_priv_key_path.is_some()
    }

    pub(crate) fn local_cert_chain_path(&self) -> Option<&str> {
        self.local_cert_chain_path.as_deref()
    }

    pub(crate) fn local_priv_key_path(&self) -> Option<&str> {
        self.local_priv_key_path.as_deref()
    }

    pub(crate) fn peer_verification(&self) -> bool {
        self.peer_verification
    }
}
