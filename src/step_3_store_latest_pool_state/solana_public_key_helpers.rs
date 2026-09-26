//! Helpers for Solana public keys (addresses).
//!
//! **What a public key is:** every account on Solana — a wallet, a pool, a
//! program, a tick array — is identified by a 32-byte public key. Humans see it
//! written in base58 (letters and digits without look-alikes such as 0/O and
//! l/I), e.g. `Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE`. Programs and
//! network messages use the raw 32 bytes.

/// The raw 32 bytes of a Solana public key (address).
pub type PublicKeyBytes = [u8; 32];

/// Turn a base58 address string into its 32 raw bytes.
///
/// Panics on invalid input, because it is only used for hard-coded addresses
/// that must be correct for the bot to work at all.
pub fn parse_base58_public_key(base58_address: &str) -> PublicKeyBytes {
    let decoded_bytes = bs58::decode(base58_address)
        .into_vec()
        .unwrap_or_else(|error| panic!("invalid public key '{base58_address}': {error}"));
    decoded_bytes.try_into().unwrap_or_else(|wrong_length: Vec<u8>| {
        panic!(
            "public key '{base58_address}' is {} bytes, expected 32",
            wrong_length.len()
        )
    })
}

/// Turn 32 raw bytes into the base58 string humans and RPC servers use.
pub fn encode_public_key_as_base58(public_key: &PublicKeyBytes) -> String {
    bs58::encode(public_key).into_string()
}

/// First and last 4 base58 characters, e.g. `Czfq..44zE`, so log lines stay short.
pub fn shorten_public_key_for_logs(public_key: &PublicKeyBytes) -> String {
    let full = encode_public_key_as_base58(public_key);
    if full.len() <= 10 {
        return full;
    }
    format!("{}..{}", &full[..4], &full[full.len() - 4..])
}

/// Convert a byte slice from a network message into a public key, if it is exactly 32 bytes.
pub fn public_key_from_byte_slice(bytes: &[u8]) -> Option<PublicKeyBytes> {
    <PublicKeyBytes>::try_from(bytes).ok()
}
