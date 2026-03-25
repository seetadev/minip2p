#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "ed25519")]
mod ed25519;
mod key;
mod peer_id;
#[cfg(feature = "ed25519")]
mod signed;

#[cfg(feature = "ed25519")]
pub use ed25519::{
    Ed25519Keypair, ED25519_PUBLIC_KEY_LENGTH, ED25519_SECRET_KEY_LENGTH, ED25519_SIGNATURE_LENGTH,
};
#[cfg(feature = "ed25519")]
pub use key::VerifyError;
pub use key::{KeyType, PublicKey, PublicKeyError};
pub use peer_id::{PeerId, PeerIdError, PeerMultihash, VarintError, PEER_ID_MULTIHASH_SIZE};
#[cfg(feature = "ed25519")]
pub use signed::SignedBytes;
