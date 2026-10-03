use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ReadySegment {
    pub execute_num: u64,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct ExifToolResult {
    pub data: Option<Vec<serde_json::Map<String, Value>>>,
    pub error: Option<String>,
}

pub fn extract_ready_segments(buffer: &str) -> (Vec<ReadySegment>, String) {
    let mut completed = Vec::new();
    let mut remaining = buffer.to_string();

    loop {
        let Some(start) = remaining.find("{ready") else {
            break;
        };
        let after = &remaining[start + "{ready".len()..];
        let Some(end_rel) = after.find('}') else {
            break;
        };
        let num_str = &after[..end_rel];
        let Ok(execute_num) = num_str.parse::<u64>() else {
            // Skip malformed marker
            remaining = remaining[start + 1..].to_string();
            continue;
        };
        let marker_end = start + "{ready".len() + end_rel + 1;
        let output = remaining[..start].trim().to_string();
        remaining = remaining[marker_end..].trim_start().to_string();
        completed.push(ReadySegment {
            execute_num,
            output,
        });
    }

    (completed, remaining)
}

pub fn parse_exiftool_output(raw: &str) -> ExifToolResult {
    if raw.is_empty() {
        return ExifToolResult {
            data: None,
            error: None,
        };
    }

    let trimmed = raw.trim_start();
    let is_json = trimmed.starts_with('[') || trimmed.starts_with('{');

    if is_json {
        match serde_json::from_str::<Value>(raw) {
            Ok(Value::Array(items)) => {
                if let Some(Value::Object(first)) = items.first() {
                    if let Some(err) = first.get("Error") {
                        return ExifToolResult {
                            data: None,
                            error: Some(err.as_str().unwrap_or("ExifTool error").to_string()),
                        };
                    }
                }
                let maps = items
                    .into_iter()
                    .filter_map(|v| match v {
                        Value::Object(m) => Some(m),
                        _ => None,
                    })
                    .collect();
                ExifToolResult {
                    data: Some(maps),
                    error: None,
                }
            }
            Ok(Value::Object(m)) => ExifToolResult {
                data: Some(vec![m]),
                error: None,
            },
            Ok(_) => ExifToolResult {
                data: None,
                error: Some("Unexpected ExifTool JSON shape".into()),
            },
            Err(err) => ExifToolResult {
                data: None,
                error: Some(format!("Failed to parse ExifTool output: {err}")),
            },
        }
    } else if raw.to_ascii_lowercase().contains("error") {
        ExifToolResult {
            data: None,
            error: Some(raw.trim().to_string()),
        }
    } else {
        ExifToolResult {
            data: None,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ready_markers() {
        let (segs, rem) = extract_ready_segments("[{\"A\":1}]\n{ready0}\n");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].execute_num, 0);
        assert!(segs[0].output.contains("A"));
        assert!(rem.is_empty());
    }
}
