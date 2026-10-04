use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OutcomeKind {
    Cleaned,
    AlreadyClean,
    Unchanged,
    Refused,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSummary {
    pub before_count: usize,
    pub after_count: usize,
    pub removed_count: usize,
    pub still_present_count: usize,
}

pub fn summarize_metadata_change(
    before: &serde_json::Map<String, serde_json::Value>,
    after: &serde_json::Map<String, serde_json::Value>,
) -> MetadataSummary {
    let before_count = before.len();
    let after_count = after.len();
    let removed_count = before.keys().filter(|k| !after.contains_key(*k)).count();
    MetadataSummary {
        before_count,
        after_count,
        removed_count,
        still_present_count: before_count.saturating_sub(removed_count),
    }
}

pub fn classify_outcome(summary: &MetadataSummary, wrote_file: bool) -> OutcomeKind {
    if summary.removed_count > 0 {
        OutcomeKind::Cleaned
    } else if !wrote_file {
        OutcomeKind::AlreadyClean
    } else {
        OutcomeKind::Unchanged
    }
}
