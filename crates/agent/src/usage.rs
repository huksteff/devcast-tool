//! What a run cost, as the agent counts it at its end: the model and the tokens. Every step of
//! a run reads the whole conversation again; the agent keeps it in its cache, so most of what
//! it reads is `cache_read` (cheap), and the new work is input, cache writes and output.

use serde_json::Value;

/// The model and the tokens of one run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    /// The model that did most of the work (`claude-opus-5-5`); none when the agent does not
    /// say.
    pub model: Option<String>,
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Usage {
    /// The tokens of new work: input, cache writes and output.
    pub fn work(&self) -> u64 {
        self.input + self.cache_write + self.output
    }

    /// Of the `result` line of Claude Code: `modelUsage` per model, else the summed `usage`.
    pub fn claude(result: &Value) -> Option<Self> {
        if let Some(models) = result["modelUsage"].as_object().filter(|m| !m.is_empty()) {
            let mut usage = Self::default();
            let mut most = 0;
            for (model, counts) in models {
                let output = count(&counts["outputTokens"]);
                usage.input += count(&counts["inputTokens"]);
                usage.output += output;
                usage.cache_write += count(&counts["cacheCreationInputTokens"]);
                usage.cache_read += count(&counts["cacheReadInputTokens"]);
                if output >= most {
                    most = output;
                    usage.model = Some(model.clone());
                }
            }
            return Some(usage);
        }
        let summed = &result["usage"];
        summed.is_object().then(|| Self {
            model: None,
            input: count(&summed["input_tokens"]),
            output: count(&summed["output_tokens"]),
            cache_write: count(&summed["cache_creation_input_tokens"]),
            cache_read: count(&summed["cache_read_input_tokens"]),
        })
    }

    /// Of the `turn.completed` line of Codex: its input counts the cached part too.
    pub fn codex(turn: &Value) -> Option<Self> {
        let usage = &turn["usage"];
        let cached = count(&usage["cached_input_tokens"]);
        usage.is_object().then(|| Self {
            model: None,
            input: count(&usage["input_tokens"]).saturating_sub(cached),
            output: count(&usage["output_tokens"]),
            cache_write: 0,
            cache_read: cached,
        })
    }
}

/// `claude-opus-5-5` → `Opus 5.5`, `claude-sonnet-5-5-20261020` → `Sonnet 5.5`; another name
/// as it is.
pub fn model_name(model: &str) -> String {
    let Some(rest) = model.strip_prefix("claude-") else {
        return model.to_owned();
    };
    let mut parts = rest.split('-');
    let Some(family) = parts.next() else {
        return model.to_owned();
    };
    let version: Vec<&str> = parts.take_while(|p| p.len() <= 2).collect();
    let mut name: String = family
        .chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
        .collect();
    if !version.is_empty() {
        name.push(' ');
        name.push_str(&version.join("."));
    }
    name
}

/// How full the limits of the user's subscription are, 0..1, as Claude Code tells it
/// (`rate_limit_event`): the 5-hour window and the 7-day one. They count the whole account,
/// other sessions running meanwhile too.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Limits {
    pub five_hour: Option<f64>,
    pub seven_day: Option<f64>,
}

impl Limits {
    /// Of a `rate_limit_event` line; none when it has no windows.
    pub fn claude(event: &Value) -> Option<Self> {
        let windows = &event["rate_limit_info"]["unifiedWindows"];
        let limits = Self {
            five_hour: windows["five_hour"]["utilization"].as_f64(),
            seven_day: windows["seven_day"]["utilization"].as_f64(),
        };
        (limits.five_hour.is_some() || limits.seven_day.is_some()).then_some(limits)
    }
}

fn count(value: &Value) -> u64 {
    value.as_u64().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_counts_per_model() {
        // The end of a run of Claude Code 2.1.309 (owner's run, 2026-11-04).
        let result: Value = serde_json::from_str(
            r#"{"type":"result","usage":{"input_tokens":2,"output_tokens":751},
            "modelUsage":{"claude-opus-5-5":{"inputTokens":46,"outputTokens":7061,
            "cacheReadInputTokens":547212,"cacheCreationInputTokens":34268}}}"#,
        )
        .unwrap();
        let usage = Usage::claude(&result).unwrap();
        assert_eq!(usage.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!((usage.input, usage.output), (46, 7061));
        assert_eq!((usage.cache_write, usage.cache_read), (34268, 547212));
        assert_eq!(usage.work(), 46 + 34268 + 7061);
        let summed: Value =
            serde_json::from_str(r#"{"usage":{"input_tokens":5,"output_tokens":9}}"#).unwrap();
        assert_eq!(Usage::claude(&summed).unwrap().output, 9);
        assert_eq!(Usage::claude(&Value::Null), None);
    }

    #[test]
    fn codex_counts_its_cached_input_apart() {
        let turn: Value = serde_json::from_str(
            r#"{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50}}"#,
        )
        .unwrap();
        let usage = Usage::codex(&turn).unwrap();
        assert_eq!(
            (usage.input, usage.cache_read, usage.output),
            (200, 800, 50)
        );
    }

    #[test]
    fn model_ids_read_as_names() {
        assert_eq!(model_name("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_name("claude-sonnet-5-5-20261020"), "Sonnet 5.5");
        assert_eq!(model_name("claude-haiku-5-5"), "Haiku 5.5");
        assert_eq!(model_name("gpt-5"), "gpt-5");
    }
}
