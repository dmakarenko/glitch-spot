// ABOUTME: Makes the glitchspot-pair link that the Desktop app shows as a QR code for an iPhone.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// RFC 3986 unreserved characters stay; every other byte is percent-encoded.
const NOT_UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub fn link(
    receive_url: &str,
    sender_id: &str,
    sender_secret: &[u8; 32],
    desktop_public_key: &[u8; 32],
) -> String {
    format!(
        "glitchspot-pair://v1?url={}&sender={}&secret={}&key={}",
        utf8_percent_encode(receive_url, NOT_UNRESERVED),
        utf8_percent_encode(sender_id, NOT_UNRESERVED),
        URL_SAFE_NO_PAD.encode(sender_secret),
        URL_SAFE_NO_PAD.encode(desktop_public_key),
    )
}
