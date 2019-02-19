use std::collections::HashSet;

pub fn log_missing_domains(
    expected: &HashSet<String>,
    actual: &HashSet<String>,
    provider: &str,
    domain: &str,
) -> usize {
    let missing_domains = expected
        .difference(actual)
        .cloned()
        .collect::<Vec<String>>();

    if !missing_domains.is_empty() {
        warn!(
            "records not found in {} domain {}: {}",
            provider,
            domain,
            missing_domains.join(", ")
        );
    }

    missing_domains.len()
}
