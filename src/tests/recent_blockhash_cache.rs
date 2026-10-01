use crate::solana_connections::is_newer_slot;

/// A blockhash from an older or equal slot must not replace the one already cached.
#[test]
fn only_a_later_slot_replaces_the_cache() {
    assert!(is_newer_slot(None, 1));
    assert!(is_newer_slot(Some(10), 11));
    assert!(!is_newer_slot(Some(10), 10));
    assert!(!is_newer_slot(Some(10), 9));
}
