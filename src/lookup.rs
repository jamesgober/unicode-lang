//! Binary-search helpers over the generated range tables.
//!
//! Every table is a sorted, non-overlapping list of inclusive code point
//! ranges, so membership and value lookups are a single `partition_point`
//! search — branch-predictable and cache-friendly, with no allocation.

/// Returns `true` when `cp` falls inside any `(start, end)` range in `table`.
#[inline]
pub(crate) fn in_ranges(cp: u32, table: &[(u32, u32)]) -> bool {
    // Index of the first range whose end is >= cp; that is the only range that
    // can contain cp, since ranges are disjoint and ascending.
    let idx = table.partition_point(|&(_, end)| end < cp);
    matches!(table.get(idx), Some(&(start, _)) if cp >= start)
}

/// Returns the per-range `u8` payload for `cp` (combining class or quick-check
/// flag), or `0` when `cp` is not covered by any range.
#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn range_value(cp: u32, table: &[(u32, u32, u8)]) -> u8 {
    // This helper only serves the combining-class and quick-check tables, and
    // no ASCII scalar has a non-zero combining class or a non-`Yes` quick-check
    // value. The sub-0x80 fast path is therefore exact and skips the search for
    // the overwhelmingly common case.
    if cp < 0x80 {
        return 0;
    }
    let idx = table.partition_point(|&(_, end, _)| end < cp);
    match table.get(idx) {
        Some(&(start, _, value)) if cp >= start => value,
        _ => 0,
    }
}
