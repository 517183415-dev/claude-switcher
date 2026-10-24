//! A headless installation probe runs before Tauri, account storage or lifecycle initialization.
use switcher_domain::Edition;

fn parse(args: &[String]) -> Result<(&str, &str), &'static str> {
    if args.len() != 3 || args[0] != "--installation-health" {
        return Err("installation_health_arguments");
    }
    for (value, length) in [(&args[1], 32), (&args[2], 64)] {
        if value.len() != length
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("installation_health_arguments");
        }
    }
    Ok((&args[1], &args[2]))
}

/// None leaves ordinary startup unchanged. Probe stdout is diagnostic only.
pub fn run(args: &[String], edition: Edition) -> Option<i32> {
    if args.first().map(String::as_str) != Some("--installation-health") {
        return None;
    }
    let parsed = parse(args);
    let bad_arguments = parsed.is_err();
    let result = parsed.and_then(|(generation, nonce)| {
        switcher_core::installation_health::gui_probe(edition, generation, nonce)
    });
    println!(
        "{}",
        serde_json::json!({
            "ok": result.is_ok(),
            "code": result.err().unwrap_or("installation_health_reported"),
            "diagnostic_only": true,
        })
    );
    Some(if bad_arguments {
        64
    } else if result.is_ok() {
        0
    } else {
        78
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Vec<String> {
        vec![
            "--installation-health".into(),
            "a".repeat(32),
            "b".repeat(64),
        ]
    }
    #[test]
    fn only_exact_health_arguments_are_accepted() {
        let valid = args();
        assert_eq!(parse(&valid), Ok((valid[1].as_str(), valid[2].as_str())));
        for malformed in [
            vec![],
            vec![valid[0].clone()],
            valid[..2].to_vec(),
            [valid.clone(), vec!["extra".into()]].concat(),
        ] {
            assert_eq!(parse(&malformed), Err("installation_health_arguments"));
        }
        for (index, bad) in [
            (0, "--other".into()),
            (1, "A".repeat(32)),
            (2, "b".repeat(63)),
            (2, "/".repeat(64)),
        ] {
            let mut malformed = args();
            malformed[index] = bad;
            assert_eq!(parse(&malformed), Err("installation_health_arguments"));
        }
    }
    #[test]
    fn ordinary_startup_is_not_a_probe() {
        assert_eq!(run(&[], Edition::Personal), None);
        assert_eq!(run(&["--version".into()], Edition::Public), None);
    }
}
