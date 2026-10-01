use serde_json::Value;

pub const MAX_METADATA_BYTES: usize = 8192;

/// Bound the PostgreSQL jsonb text representation, including separator spaces.
pub fn valid_metadata(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if !object.iter().all(|(key, value)| {
        !key.contains('\0') && value.as_str().is_some_and(|s| !s.contains('\0'))
    }) {
        return false;
    }
    let separators = object.len().saturating_mul(2).saturating_sub(1);
    serde_json::to_vec(value)
        .is_ok_and(|json| json.len().saturating_add(separators) <= MAX_METADATA_BYTES)
}
