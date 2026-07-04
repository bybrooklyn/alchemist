//! Secret redaction for anything that gets persisted to the log store or
//! emitted to operators. Heuristic and conservative: it targets known
//! token-bearing keys and webhook URL shapes rather than trying to catch
//! everything. Dependency-free (no regex) and length-preserving on ASCII so
//! byte offsets stay aligned with the lowercased scan copy.

/// Keys whose value is treated as a secret in `key=value` / `key: value` form
/// (query params, cookies, headers, config dumps). Lower-case; matched
/// case-insensitively at a word boundary.
const SENSITIVE_KEYS: &[&str] = &[
    "authorization",
    "alchemist_session",
    "access_token",
    "refresh_token",
    "client_secret",
    "webhook_token",
    "x-api-key",
    "api_key",
    "apikey",
    "api-key",
    "password",
    "passwd",
    "secret",
    "token",
    "bearer",
];

const MASK: &str = "***";

/// Redact likely secrets from a string.
pub fn redact_secrets(input: &str) -> String {
    redact_webhook_urls(&redact_key_values(input))
}

fn is_key_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_value_delimiter(b: u8, mask_through_spaces: bool) -> bool {
    // `authorization`/`bearer` values are a whole credential ("Bearer <token>")
    // and must be masked through internal spaces, stopping only at a real
    // boundary, so the token itself never leaks.
    let space = matches!(b, b' ' | b'\t');
    if space && mask_through_spaces {
        return false;
    }
    matches!(
        b,
        b' ' | b'\t'
            | b'\n'
            | b'\r'
            | b'&'
            | b','
            | b';'
            | b'"'
            | b'\''
            | b')'
            | b']'
            | b'}'
            | b'\\'
    )
}

fn utf8_len(first: u8) -> usize {
    match first {
        b if b < 0x80 => 1,
        b if b >= 0xF0 => 4,
        b if b >= 0xE0 => 3,
        b if b >= 0xC0 => 2,
        _ => 1,
    }
}

/// Masks the value following any `SENSITIVE_KEYS` entry joined by `=` or `:`.
fn redact_key_values(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let bytes = input.as_bytes();
    let mut result = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        let boundary = i == 0 || !is_key_char(bytes[i - 1]);
        if boundary
            && let Some(key) = SENSITIVE_KEYS
                .iter()
                .find(|key| lower_bytes[i..].starts_with(key.as_bytes()))
        {
            let after_key = i + key.len();
            // The match must end at a word boundary so `token` does not match
            // inside `tokenizer`.
            let ends_cleanly = after_key >= bytes.len() || !is_key_char(bytes[after_key]);
            if ends_cleanly {
                let mask_through_spaces = matches!(*key, "authorization" | "bearer");
                let mut j = after_key;
                while j < bytes.len() && bytes[j] == b' ' {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'=' || bytes[j] == b':') {
                    j += 1;
                    while j < bytes.len()
                        && (bytes[j] == b' ' || bytes[j] == b'"' || bytes[j] == b'\'')
                    {
                        j += 1;
                    }
                    let value_start = j;
                    while j < bytes.len() && !is_value_delimiter(bytes[j], mask_through_spaces) {
                        j += 1;
                    }
                    if j > value_start {
                        result.push_str(&input[i..value_start]);
                        result.push_str(MASK);
                        i = j;
                        continue;
                    }
                }
            }
        }

        let len = utf8_len(bytes[i]);
        result.push_str(&input[i..(i + len).min(bytes.len())]);
        i += len;
    }

    result
}

/// Masks the token segment of well-known webhook URLs whose secret is carried in
/// the path (Discord, Slack), which the key/value pass cannot see.
fn redact_webhook_urls(input: &str) -> String {
    input
        .split_inclusive(char::is_whitespace)
        .map(|chunk| {
            let trimmed = chunk.trim_end();
            let trailing = &chunk[trimmed.len()..];
            let lower = trimmed.to_ascii_lowercase();
            if (lower.contains("discord.com/api/webhooks/")
                || lower.contains("discordapp.com/api/webhooks/")
                || lower.contains("hooks.slack.com/services/"))
                && let Some(idx) = trimmed.rfind('/')
            {
                return format!("{}/{}{}", &trimmed[..idx], MASK, trailing);
            }
            chunk.to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_key_value_secrets() {
        assert_eq!(
            redact_secrets("GET /events?token=abc123&limit=5"),
            "GET /events?token=***&limit=5"
        );
        // The whole credential after `Authorization:` is masked, scheme included,
        // so the token never leaks.
        assert_eq!(
            redact_secrets("Authorization: Bearer sk-deadbeef"),
            "Authorization: ***"
        );
        assert_eq!(
            redact_secrets("Cookie: alchemist_session=ey.JWT.sig done"),
            "Cookie: alchemist_session=*** done"
        );
        assert_eq!(redact_secrets("password = hunter2"), "password = ***");
    }

    #[test]
    fn does_not_mask_lookalike_keys_or_empty_values() {
        assert_eq!(redact_secrets("tokenizer=fast"), "tokenizer=fast");
        assert_eq!(redact_secrets("a_token_count is 5"), "a_token_count is 5");
        // No value to mask.
        assert_eq!(redact_secrets("token="), "token=");
    }

    #[test]
    fn masks_path_embedded_webhook_tokens() {
        assert_eq!(
            redact_secrets("posting to https://discord.com/api/webhooks/12345/SECRETTOKEN now"),
            "posting to https://discord.com/api/webhooks/12345/*** now"
        );
        assert_eq!(
            redact_secrets("https://hooks.slack.com/services/T0/B0/XXXXYYYY"),
            "https://hooks.slack.com/services/T0/B0/***"
        );
    }

    #[test]
    fn leaves_plain_text_untouched() {
        let msg = "Job 12 encoding /media/Movie (2021)/movie.mkv at 42%";
        assert_eq!(redact_secrets(msg), msg);
    }
}
