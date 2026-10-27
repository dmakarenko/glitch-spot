// ABOUTME: Opens snapshots that a paired iPhone sealed with HPKE (PSK mode) to the Desktop key.
// ABOUTME: Body layout: version byte, 32-byte encapsulated key, ChaCha20-Poly1305 ciphertext.

use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem, OpModeR, PskBundle};

pub mod pairing;

pub const INFO: &[u8] = b"glitch-spot sealed snapshot v1";
pub const VERSION_BYTE: u8 = 0x01;
pub const ENCAPSULATED_KEY_BYTES: usize = 32;
pub const TAG_BYTES: usize = 16;
pub const MAX_SEALED_BYTES: usize = 25 * 1024 * 1024;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum SealError {
    #[error("sealed body is {0} bytes, more than the limit")]
    TooLarge(usize),
    #[error("sealed body is too short")]
    TooShort,
    #[error("unknown sealed body version {0}")]
    UnknownVersion(u8),
    #[error("sealed body does not open")]
    Open,
}

/// Opens one sealed body. The sender secret is the HPKE PSK and the sender id is its PSK id.
pub fn open(
    sealed: &[u8],
    desktop_private_key: &[u8; 32],
    sender_id: &str,
    sender_secret: &[u8; 32],
) -> Result<Vec<u8>, SealError> {
    if sealed.len() > MAX_SEALED_BYTES {
        return Err(SealError::TooLarge(sealed.len()));
    }
    let (&version, rest) = sealed.split_first().ok_or(SealError::TooShort)?;
    if version != VERSION_BYTE {
        return Err(SealError::UnknownVersion(version));
    }
    if rest.len() < ENCAPSULATED_KEY_BYTES + TAG_BYTES {
        return Err(SealError::TooShort);
    }
    let (encapsulated_key, ciphertext) = rest.split_at(ENCAPSULATED_KEY_BYTES);

    let private_key = <X25519HkdfSha256 as Kem>::PrivateKey::from_bytes(desktop_private_key)
        .map_err(|_| SealError::Open)?;
    let encapsulated_key = <X25519HkdfSha256 as Kem>::EncappedKey::from_bytes(encapsulated_key)
        .map_err(|_| SealError::Open)?;
    let psk = PskBundle::new(sender_secret, sender_id.as_bytes()).map_err(|_| SealError::Open)?;

    hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, X25519HkdfSha256>(
        &OpModeR::Psk(psk),
        &private_key,
        &encapsulated_key,
        INFO,
        ciphertext,
        b"",
    )
    .map_err(|_| SealError::Open)
}

#[cfg(test)]
mod tests;
