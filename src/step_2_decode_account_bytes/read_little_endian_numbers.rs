//! Read fixed-size values out of an account's byte array.
//!
//! Every function takes the whole account data and a byte offset (position),
//! and returns `None` if the data is too short — a truncated or unexpected
//! account must never crash the bot.
//!
//! Solana stores numbers little-endian (lowest byte first), so the 2 bytes
//! `90 01` mean `0x0190 = 400`.

use crate::step_3_store_latest_pool_state::PublicKeyBytes;

pub fn read_u16_little_endian(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2)?.try_into().ok().map(u16::from_le_bytes)
}

pub fn read_i32_little_endian(data: &[u8], offset: usize) -> Option<i32> {
    data.get(offset..offset + 4)?.try_into().ok().map(i32::from_le_bytes)
}

pub fn read_u128_little_endian(data: &[u8], offset: usize) -> Option<u128> {
    data.get(offset..offset + 16)?.try_into().ok().map(u128::from_le_bytes)
}

pub fn read_i128_little_endian(data: &[u8], offset: usize) -> Option<i128> {
    data.get(offset..offset + 16)?.try_into().ok().map(i128::from_le_bytes)
}

/// Public keys are stored as 32 raw bytes, no byte-order conversion needed.
pub fn read_public_key(data: &[u8], offset: usize) -> Option<PublicKeyBytes> {
    data.get(offset..offset + 32)?.try_into().ok()
}
