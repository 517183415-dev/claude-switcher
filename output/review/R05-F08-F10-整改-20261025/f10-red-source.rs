//! A deliberately small YAML subset for Clash Verge's profile enhancement.
//! Ambiguous syntax is rejected before any file is written.
use std::io;

pub(super) const GROUP_RELATIVE: &str =
    "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/clash-verge.yaml";
const BEGIN: &str = "# >>> claude-switcher pinned rules >>>";
const END: &str = "# <<< claude-switcher pinned rules <<<";
const DOMAINS: [&str; 13] = [
    "claude.ai",
    "claude.com",
    "anthropic.com",
    "claudeusercontent.com",
    "challenges.cloudflare.com",
    "datadoghq.com",
    "segment.io",
    "stripe.com",
    "stripe.network",
    "stripecdn.com",
    "intercom.io",
    "intercomcdn.com",
    "intercomassets.com",
];

fn unsupported() -> io::Error {
    io::Error::other("Clash YAML格式不受支持或有歧义")
}

fn scalar(value: &str) -> io::Result<&str> {
    let value = value.trim();
    let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2
        || value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2
    {
        &value[1..value.len() - 1]
    } else {
        value
    };
    if value.is_empty()
        || value.starts_with(['?', '-', '@', '`', '|', '>', '%'])
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off"
        )
        || value.parse::<f64>().is_ok()
        || value.contains([
            '\n', '\r', '\\', '"', '\'', ',', '#', '[', ']', '{', '}', '&', '*', '!', ':',
        ])
        || value.chars().any(char::is_control)
    {
        return Err(unsupported());
    }
    Ok(value)
}

pub(super) fn first_select_group(bytes: &[u8]) -> io::Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| unsupported())?;
    let mut in_groups = false;
    let mut groups = 0;
    let mut name: Option<String> = None;
    let mut group_type: Option<String> = None;
    let mut first_select: Option<String> = None;
    let mut entries = 0;
    let mut seen_name = false;
    let mut seen_type = false;
    let mut nested_field = false;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with(['%', '-', '{', '['])
            || line.contains("<<:")
            || (in_groups && line.contains(['&', '*', '!', '{', '}', '[', ']']))
        {
            return Err(unsupported());
        }
        if !line.starts_with(char::is_whitespace) {
            if in_groups {
                if group_type.as_deref() == Some("select") && first_select.is_none() {
                    first_select = Some(name.take().ok_or_else(unsupported)?);
                }
                in_groups = false;
            }
            if line == "proxy-groups:" {
                groups += 1;
                in_groups = true;
            }
            continue;
        }
        if !in_groups {
            continue;
        }
        let property = if let Some(rest) = line.strip_prefix("  - ") {
            if group_type.as_deref() == Some("select") && first_select.is_none() {
                first_select = Some(name.take().ok_or_else(unsupported)?);
            }
            entries += 1;
            name = None;
            group_type = None;
            seen_name = false;
            seen_type = false;
            rest
        } else if entries > 0 && line.starts_with("    ") && !line[4..].starts_with(' ') {
            &line[4..]
        } else if entries > 0 && line.starts_with("      ") {
            if !nested_field {
                return Err(unsupported());
            }
            continue;
        } else {
            return Err(unsupported());
        };
        if property.contains(['&', '*', '!', '{', '}', '[', ']']) {
            return Err(unsupported());
        }
        let (key, raw) = property.split_once(':').ok_or_else(unsupported)?;
        match key {
            "name" => {
                if seen_name {
                    return Err(unsupported());
                }
                name = Some(scalar(raw)?.to_owned());
                seen_name = true;
                nested_field = false;
            }
            "type" => {
                if seen_type {
                    return Err(unsupported());
                }
                group_type = Some(scalar(raw)?.to_owned());
                seen_type = true;
                nested_field = false;
            }
            _ if !key.is_empty()
                && key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') =>
            {
                nested_field = true;
            }
            _ => return Err(unsupported()),
        }
    }
    if in_groups && group_type.as_deref() == Some("select") && first_select.is_none() {
        first_select = Some(name.ok_or_else(unsupported)?);
    }
    if groups != 1 {
        return Err(unsupported());
    }
    first_select.ok_or_else(unsupported)
}

fn block(group: &str, newline: &str) -> String {
    let mut output = format!("  {BEGIN}{newline}");
    for domain in DOMAINS {
        // JSON strings are valid YAML double-quoted scalars. The whole Clash rule
        // is quoted, so a group name with spaces or Unicode cannot change YAML shape.
        let rule = format!("DOMAIN-SUFFIX,{domain},{group}");
        output.push_str("  - ");
        output.push_str(&serde_json::to_string(&rule).expect("string encoding"));
        output.push_str(newline);
    }
    output.push_str(&format!("  {END}{newline}"));
    output
}

pub(super) fn propose(original: Option<&[u8]>, group: &str) -> io::Result<(Vec<u8>, bool)> {
    if group.is_empty() || group.chars().any(char::is_control) {
        return Err(unsupported());
    }
    let Some(original) = original else {
        return Ok((format!("# Profile Enhancement Merge Template for Clash Verge\n\nprofile:\n  store-selected: true\n\nprepend-rules:\n{}", block(group, "\n")).into_bytes(), true));
    };
    let text = std::str::from_utf8(original).map_err(|_| unsupported())?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut clean = String::new();
    let mut in_block = false;
    let mut current_top = String::new();
    for line in text.split_inclusive('\n') {
        if line.trim() == BEGIN || line.trim() == END {
            let raw = line.trim_end_matches(['\r', '\n']);
            if raw != line.trim()
                && !(raw == format!("  {}", line.trim()) && current_top == "prepend-rules")
            {
                return Err(unsupported());
            }
        }
        if !line.starts_with(char::is_whitespace)
            && !line.starts_with('#')
            && let Some((key, _)) = line.split_once(':')
        {
            current_top = key.to_owned();
        }
        if line.trim() == BEGIN {
            if in_block {
                return Err(unsupported());
            }
            in_block = true;
        } else if line.trim() == END {
            if !in_block {
                return Err(unsupported());
            }
            in_block = false;
        } else if !in_block {
            clean.push_str(line);
        }
    }
    // An incomplete marker could hide user content; preserve it instead of deleting it.
    if in_block {
        return Err(unsupported());
    }
    let mut result = String::new();
    let mut inserted = false;
    for line in clean.split_inclusive('\n') {
        result.push_str(line);
        if !inserted && line.trim_end_matches(['\r', '\n']) == "prepend-rules:" {
            if !line.ends_with('\n') {
                result.push_str(newline);
            }
            result.push_str(&block(group, newline));
            inserted = true;
        }
    }
    if !inserted {
        result = format!(
            "prepend-rules:{newline}{}{newline}{clean}",
            block(group, newline)
        );
    }
    let changed = result.as_bytes() != original;
    Ok((result.into_bytes(), changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f10_accept_case(name: &str, original: &str, insertion: usize, has_target: bool) {
        let (updated, changed) = propose(Some(original.as_bytes()), "F10 Group").unwrap();
        let updated = String::from_utf8(updated).unwrap();
        let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
        let separator = if insertion > 0 && !original[..insertion].ends_with('\n') {
            newline
        } else {
            ""
        };
        let added = if has_target {
            format!("{separator}{}", block("F10 Group", newline))
        } else {
            format!("{separator}prepend-rules:{newline}{}{newline}", block("F10 Group", newline))
        };
        let expected = format!("{}{}{}", &original[..insertion], added, &original[insertion..]);
        println!("F10 ACCEPT {name}: changed={changed}, insertion={insertion}, managed_rules={}", updated.matches("DOMAIN-SUFFIX,").count());
        assert_eq!(updated, expected, "{name}: original bytes and insertion position");
        assert!(changed, "{name}");
        assert_eq!(updated.matches("DOMAIN-SUFFIX,").count(), 13, "{name}");
        assert_eq!(updated.lines().filter(|line| line.starts_with("prepend-rules:")).count(), 1, "{name}");
        let (again, second_changed) = propose(Some(updated.as_bytes()), "F10 Group").unwrap();
        println!("F10_FIXTURE {}", serde_json::json!({"name": name, "original": original, "output": updated, "expected": "accept", "accepted": true, "changed": changed, "second_output": String::from_utf8_lossy(&again), "second_changed": second_changed, "group": "F10 Group"}));
        assert!(!second_changed, "{name}: second pass changed");
        assert_eq!(again, updated.as_bytes(), "{name}: idempotence");
    }

    #[test]
    fn f10_accept_target_header_suffixes_and_preserve_user_bytes() {
        for (name, header) in [
            ("plain", "prepend-rules:\n"),
            ("spaces", "prepend-rules:   \n"),
            ("tabs", "prepend-rules:\t \n"),
            ("comment", "prepend-rules: # keep comment\n"),
            ("crlf-comment", "prepend-rules: \t# keep comment\r\n"),
            ("no-final-newline", "prepend-rules: # keep comment"),
        ] {
            let newline = if header.contains("\r\n") { "\r\n" } else { "\n" };
            let prefix = format!("# user prefix{newline}profile: &anchor{newline}  note: keep{newline}");
            let suffix = if header.ends_with('\n') {
                format!("  # user comment{newline}{newline}  - USER,RULE{newline}rules:{newline}  - MATCH,DIRECT{newline}")
            } else { String::new() };
            let original = format!("{prefix}{header}{suffix}");
            f10_accept_case(name, &original, prefix.len() + header.len(), true);
        }
    }

    #[test]
    fn f10_document_header_stays_before_inserted_rules() {
        for prefix in ["---\n", "# keep\n\n--- # document\n", "# keep\r\n---\r\n", "---"] {
            let suffix = if prefix.ends_with('\n') { "profile: &anchor\n  store-selected: true\n" } else { "" };
            f10_accept_case(prefix, &format!("{prefix}{suffix}"), prefix.len(), false);
        }
    }

    #[test]
    fn f10_nested_target_and_scalar_text_remain_unchanged() {
        let nested = "# prepend-rules: is only a comment\nprofile: &anchor\n  prepend-rules: []\n  note: |\n    prepend-rules: []\n    ---\n    ...\n";
        f10_accept_case("nested-only", nested, 0, false);
        let header = "prepend-rules: # actual target\n";
        f10_accept_case("nested-before-target", &format!("{nested}{header}  - USER,RULE\n"), nested.len() + header.len(), true);
    }

    #[test]
    fn f10_reject_comment_without_yaml_separator() {
        let original = "prepend-rules:#comment\n  - RULE\n";
        let result = propose(Some(original.as_bytes()), "Group");
        println!("F10_FIXTURE {}", serde_json::json!({"name": "missing-comment-separator", "original": original, "expected": "reject", "accepted": result.is_ok(), "output": result.as_ref().ok().map(|(bytes, _)| String::from_utf8_lossy(bytes)), "changed": result.as_ref().ok().map(|(_, changed)| *changed)}));
        assert!(result.is_err());
    }

    #[test]
    fn f10_reject_inline_values() {
        for value in ["[]", "[RULE]", "{}", "null", "&anchor", "*alias", "|", ">", "!!seq", "RULE"] {
            let original = format!("prepend-rules: {value}\n");
            let result = propose(Some(original.as_bytes()), "Group");
            println!("F10_FIXTURE {}", serde_json::json!({"original": original, "expected": "reject", "accepted": result.is_ok(), "output": result.as_ref().ok().map(|(bytes, _)| String::from_utf8_lossy(bytes)), "changed": result.as_ref().ok().map(|(_, changed)| *changed)}));
            println!("F10 REJECT inline={value:?}: rejected={}", result.is_err());
            assert!(result.is_err(), "inline={value:?}");
        }
    }

    #[test]
    fn f10_reject_duplicate_keys_and_document_ambiguity() {
        for original in [
            "prepend-rules:\nprepend-rules:\n",
            "prepend-rules: # first\nprepend-rules: \t# second\n",
            "---\nprofile:\n---\nprofile:\n",
            "profile:\n---\n",
            "...\n",
            "--- content\n",
            "%YAML 1.2\n---\n",
            "'prepend-rules':\n",
            "\"prepend-rules\":\n",
            "prepend-rules :\n",
            "? prepend-rules\n: []\n",
            "{prepend-rules: []}\n",
            "prepend-rules\n",
        ] {
            let result = propose(Some(original.as_bytes()), "Group");
            println!("F10_FIXTURE {}", serde_json::json!({"original": original, "expected": "reject", "accepted": result.is_ok(), "output": result.as_ref().ok().map(|(bytes, _)| String::from_utf8_lossy(bytes)), "changed": result.as_ref().ok().map(|(_, changed)| *changed)}));
            println!("F10 REJECT structure={original:?}: rejected={}", result.is_err());
            assert!(result.is_err(), "structure={original:?}");
        }
    }

    #[test]
    fn f10_reject_ambiguous_markers_and_keep_legacy_migration() {
        for original in [
            format!("{BEGIN}\n"),
            format!("{END}\n"),
            format!("{BEGIN}\n{BEGIN}\n{END}\n"),
            format!("profile:\n  {BEGIN}\n  {END}\n"),
            format!("profile:\n  note: |\n    {BEGIN}\n    {END}\n"),
            format!("prepend-rules:\n  {BEGIN} extra\n  {END}\n"),
            format!("prepend-rules:\n  {BEGIN}\n  {END} extra\n"),
        ] {
            let result = propose(Some(original.as_bytes()), "Group");
            println!("F10_FIXTURE {}", serde_json::json!({"original": original, "expected": "reject", "accepted": result.is_ok(), "output": result.as_ref().ok().map(|(bytes, _)| String::from_utf8_lossy(bytes)), "changed": result.as_ref().ok().map(|(_, changed)| *changed)}));
            println!("F10 REJECT marker={original:?}: rejected={}", result.is_err());
            assert!(result.is_err(), "marker={original:?}");
        }
        let user = "profile: &anchor\n  store-selected: true\nprepend-rules: # user\n  - KEEP\n";
        let legacy = format!("{BEGIN}\nprepend-rules:\n  - OLD\n{END}\n{user}");
        let (updated, changed) = propose(Some(legacy.as_bytes()), "Group").unwrap();
        let updated = String::from_utf8(updated).unwrap();
        assert!(updated.starts_with("profile: &anchor\n  store-selected: true\nprepend-rules: # user\n"));
        assert!(updated.ends_with("  - KEEP\n"));
        assert!(!updated.contains("OLD"));
        assert_eq!(updated.lines().filter(|line| line.starts_with("prepend-rules:")).count(), 1);
        let (again, second_changed) = propose(Some(updated.as_bytes()), "Group").unwrap();
        assert!(!second_changed);
        assert_eq!(again, updated.as_bytes());
        println!("F10_FIXTURE {}", serde_json::json!({"name": "legacy-migration", "original": legacy, "output": updated, "expected": "accept", "accepted": true, "changed": changed, "second_output": String::from_utf8_lossy(&again), "second_changed": second_changed, "group": "Group"}));
        println!("F10 ACCEPT legacy migration: one target; anchor/user preserved; idempotent");
    }

    #[test]
    fn select_group_subset_and_ambiguous_rejection() {
        assert_eq!(first_select_group("mixed-port: 7890\nproxy-groups:\n  - name: 普通组\n    type: fallback\n  - name: '香港 节点'\n    type: select\n    proxies:\n      - DIRECT\n".as_bytes()).unwrap(), "香港 节点");
        assert_eq!(
            first_select_group(b"proxy-groups:\n  - name: \"Space Group\"\n    type: select\n")
                .unwrap(),
            "Space Group"
        );
        for bad in [
            "proxy-groups: [{name: A, type: select}]",
            "proxy-groups:\n  - name: A\n    name: B\n    type: select\n",
            "proxy-groups:\n  - name: &x A\n    type: select\n",
            "proxy-groups:\n  - name: A,B\n    type: select\n",
            "proxy-groups:\n  - name: A\n    type: select\nproxy-groups:\n  - name: B\n    type: select\n",
            "proxy-groups:\n  - name: Hong Kong\n      Group\n    type: select\n",
            "proxy-groups:\n  - name: Hong Kong\n    type: select\n      extra\n",
        ] {
            assert!(first_select_group(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn create_replace_and_preserve_user_rules() {
        let (created, changed) = propose(None, "香港 节点").unwrap();
        assert!(changed);
        assert_eq!(
            String::from_utf8_lossy(&created)
                .matches("DOMAIN-SUFFIX,")
                .count(),
            13
        );
        assert!(!propose(Some(&created), "香港 节点").unwrap().1);
        let old = format!(
            "profile:\n  store-selected: true\nprepend-rules:\n  {BEGIN}\n  - DOMAIN-SUFFIX,claude.ai,OLD\n  {END}\n  - DOMAIN-SUFFIX,user.example,DIRECT\n"
        );
        let (updated, _) = propose(Some(old.as_bytes()), "香港 节点").unwrap();
        let updated = String::from_utf8(updated).unwrap();
        assert!(updated.contains("DOMAIN-SUFFIX,user.example,DIRECT"));
        assert!(!updated.contains(",OLD"));
        assert!(!propose(Some(updated.as_bytes()), "香港 节点").unwrap().1);
        assert!(propose(Some(format!("{BEGIN}\n").as_bytes()), "A").is_err());
        let nested_first = b"profile:\n  prepend-rules:\n    - KEEP\nprepend-rules:\n  - DOMAIN-SUFFIX,user.example,DIRECT\n";
        let (updated, _) = propose(Some(nested_first), "香港 节点").unwrap();
        let updated = String::from_utf8(updated).unwrap();
        assert!(
            updated.starts_with("profile:\n  prepend-rules:\n    - KEEP\nprepend-rules:\n  # >>>")
        );
        assert!(updated.contains("  - DOMAIN-SUFFIX,user.example,DIRECT"));
        let literal = format!("profile:\n  note: |\n    {BEGIN}\n    USER CONTENT\n    {END}\n");
        assert!(propose(Some(literal.as_bytes()), "A").is_err());
        let complex = b"---\nprofile: &anchor\n  store-selected: true\n";
        let (updated, _) = propose(Some(complex), "Group $literal").unwrap();
        assert!(updated.ends_with(complex));
        let legacy = format!(
            "{BEGIN}\nprepend-rules:\n  - DOMAIN-SUFFIX,claude.ai,OLD\n{END}\nprofile:\n  store-selected: true\n"
        );
        let (updated, _) = propose(Some(legacy.as_bytes()), "香港 节点").unwrap();
        assert_eq!(
            String::from_utf8_lossy(&updated)
                .matches("prepend-rules:")
                .count(),
            1
        );
    }
}
