/// Kebab-case an entity name for use as a filename, e.g. `UserService` ->
/// `user-service`. CLAUDE.md's convention: stable across renames by
/// tracking the symbol's graph id, not its filename — full rename
/// remapping is deferred (see Phase 2's DECISION on graph/build.rs), so
/// for now the slug is simply derived from the current name.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut prev_lower_or_digit = false;

    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if prev_lower_or_digit {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
            prev_lower_or_digit = false;
        } else if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_lower_or_digit = true;
        } else {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            prev_lower_or_digit = false;
        }
    }

    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_pascal_case() {
        assert_eq!(slugify("UserService"), "user-service");
        assert_eq!(slugify("PolicyService"), "policy-service");
    }

    #[test]
    fn slugifies_already_lowercase_names() {
        assert_eq!(slugify("calculatePremium"), "calculate-premium");
    }

    #[test]
    fn handles_consecutive_uppercase_and_separators() {
        assert_eq!(slugify("HTTPClient"), "httpclient");
        assert_eq!(slugify("user_service"), "user-service");
    }
}
