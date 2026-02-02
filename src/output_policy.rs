use crate::session::SessionDefaults;
use crate::tools::{ToolResponse, ToolResponseContent};

const DEFAULT_MAX_LINES: usize = 120;
const DEFAULT_MAX_CHARS: usize = 8000;

pub fn apply_output_policy(mut response: ToolResponse, defaults: &SessionDefaults) -> ToolResponse {
    let verbosity = defaults
        .verbosity
        .as_deref()
        .unwrap_or("terse")
        .trim()
        .to_lowercase();
    let is_terse = verbosity != "verbose";
    let include_next_steps = defaults.include_next_steps.unwrap_or(!is_terse);
    let max_lines = defaults
        .max_output_lines
        .map(|value| value as usize)
        .or_else(|| if is_terse { Some(DEFAULT_MAX_LINES) } else { None });
    let max_chars = defaults
        .max_output_chars
        .map(|value| value as usize)
        .or_else(|| if is_terse { Some(DEFAULT_MAX_CHARS) } else { None });

    for content in response.content.iter_mut() {
        let ToolResponseContent::Text { text } = content;
        let mut filtered = if include_next_steps {
            text.clone()
        } else {
            strip_next_steps(text)
        };
        filtered = truncate_text(&filtered, max_lines, max_chars);
        *text = filtered;
    }

    response
}

fn strip_next_steps(text: &str) -> String {
    let markers = [
        "\nNext steps:",
        "\nNext Steps:",
        "\nNext steps",
        "\nNext Steps",
        "Next steps:",
        "Next Steps:",
    ];
    let mut cut_idx: Option<usize> = None;
    for marker in markers {
        if let Some(idx) = text.find(marker) {
            cut_idx = Some(cut_idx.map_or(idx, |current| current.min(idx)));
        }
    }

    match cut_idx {
        Some(idx) => text[..idx].trim_end().to_string(),
        None => text.to_string(),
    }
}

fn truncate_text(text: &str, max_lines: Option<usize>, max_chars: Option<usize>) -> String {
    let mut truncated = text.to_string();
    let mut was_truncated = false;

    if let Some(limit) = max_lines {
        let lines: Vec<&str> = truncated.lines().collect();
        if lines.len() > limit && limit > 0 {
            was_truncated = true;
            if limit <= 3 {
                truncated = lines[lines.len() - limit..].join("\n");
            } else {
                let tail_count = limit - 2;
                let head = lines[0];
                let tail = &lines[lines.len() - tail_count..];
                let mut combined = String::new();
                combined.push_str(head);
                combined.push('\n');
                combined.push_str("[...truncated...]\n");
                combined.push_str(&tail.join("\n"));
                truncated = combined;
            }
        }
    }

    if let Some(limit) = max_chars {
        if truncated.len() > limit && limit > 0 {
            was_truncated = true;
            if limit <= 10 {
                truncated = truncated[truncated.len() - limit..].to_string();
            } else {
                let head_len = 10.min(limit / 4);
                let tail_len = limit - head_len - 16;
                let head = &truncated[..head_len];
                let tail = &truncated[truncated.len() - tail_len..];
                truncated = format!("{}[...]...truncated...{}", head, tail);
            }
        }
    }

    if was_truncated {
        truncated
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolResponse;

    #[test]
    fn strip_next_steps_removes_section() {
        let text = "Line 1\nNext steps:\n1. Do this\n2. Do that";
        assert_eq!(strip_next_steps(text), "Line 1");
    }

    #[test]
    fn truncate_lines_keeps_head_and_tail() {
        let text = (1..=10).map(|v| format!("L{}", v)).collect::<Vec<_>>().join("\n");
        let result = truncate_text(&text, Some(5), None);
        assert!(result.contains("L1"));
        assert!(result.contains("L10"));
        assert!(result.contains("[...truncated...]"));
    }

    #[test]
    fn apply_output_policy_terse_hides_next_steps() {
        let mut response = ToolResponse::text(
            "Result ok\nNext steps:\n1. Do".to_string(),
            true,
        );
        let defaults = SessionDefaults {
            verbosity: Some("terse".to_string()),
            include_next_steps: None,
            max_output_lines: None,
            max_output_chars: None,
            ..SessionDefaults::default()
        };
        response = apply_output_policy(response, &defaults);
        let text = match &response.content[0] {
            ToolResponseContent::Text { text } => text.clone(),
        };
        assert_eq!(text, "Result ok");
    }
}
