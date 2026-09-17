pub fn u16_le(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2)?
        .try_into()
        .ok()
        .map(u16::from_le_bytes)
}

pub fn i32_le(data: &[u8], offset: usize) -> Option<i32> {
    data.get(offset..offset + 4)?
        .try_into()
        .ok()
        .map(i32::from_le_bytes)
}

pub fn u128_le(data: &[u8], offset: usize) -> Option<u128> {
    data.get(offset..offset + 16)?
        .try_into()
        .ok()
        .map(u128::from_le_bytes)
}

pub fn i128_le(data: &[u8], offset: usize) -> Option<i128> {
    data.get(offset..offset + 16)?
        .try_into()
        .ok()
        .map(i128::from_le_bytes)
}

pub fn pubkey(data: &[u8], offset: usize) -> Option<[u8; 32]> {
    data.get(offset..offset + 32)?.try_into().ok()
}
