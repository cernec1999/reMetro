/// Return `true` if string has 1 to 3 dashes
#[inline]
pub fn is_dashed(s: &str) -> bool {
    matches!(s, "-" | "--" | "---")
}
