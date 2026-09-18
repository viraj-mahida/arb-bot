pub fn parse_pubkey(s: &str) -> [u8; 32] {
    let bytes = bs58::decode(s)
        .into_vec()
        .unwrap_or_else(|e| panic!("invalid pubkey '{s}': {e}"));
    bytes
        .try_into()
        .unwrap_or_else(|b: Vec<u8>| panic!("pubkey '{s}' is {} bytes, expected 32", b.len()))
}

pub fn encode_pubkey(bytes: &[u8; 32]) -> String {
    bs58::encode(bytes).into_string()
}

/// First/last 4 chars of base58, e.g. `Czfq..44zE`. Logs stay readable.
pub fn short_pubkey(bytes: &[u8; 32]) -> String {
    let s = encode_pubkey(bytes);
    if s.len() <= 10 {
        return s;
    }
    format!("{}..{}", &s[..4], &s[s.len() - 4..])
}

pub fn pubkey_from_slice(bytes: &[u8]) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok()
}
