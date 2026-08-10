//! Shared helpers for comparing dotted version ids like "2", "2.1", "2.1.3".
//! Port of `qt-legacy/src/core/VersionId.h`.

pub fn key(version_id: &str) -> Vec<i64> {
    version_id
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.parse::<i64>().unwrap_or(0))
        .collect()
}

pub fn less_than(left: &str, right: &str) -> bool {
    let left_key = key(left);
    let right_key = key(right);
    let max_size = left_key.len().max(right_key.len());
    for i in 0..max_size {
        let lv = left_key.get(i).copied().unwrap_or(-1);
        let rv = right_key.get(i).copied().unwrap_or(-1);
        if lv == rv {
            continue;
        }
        return lv < rv;
    }
    left < right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_dotted_ids() {
        assert!(less_than("1", "2"));
        assert!(less_than("1", "1.1"));
        assert!(less_than("1.1", "1.2"));
        assert!(less_than("1.9", "1.10"));
        assert!(less_than("1.1", "2"));
        assert!(!less_than("2", "1.9"));
        assert!(!less_than("2", "2"));
    }
}
