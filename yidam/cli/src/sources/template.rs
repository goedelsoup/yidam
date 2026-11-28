//! The `{slot}`s in an address template: which ones it names, and filling them in.
//!
//! A pack's scheme, a pin read, a lookup listing and a catalog entry's `url_template` all
//! write an address with named slots. [`super`], [`super::pinning`] and [`super::resolve`]
//! bind them, and so does [`crate::cmd::catalog::location`], where these two functions were
//! written. They live here because they are pure string work with no transport in it, and a
//! source pack reader that reached into the command layer for them made the dependency run
//! both ways (#1635).

/// The slot names in a template, in the order they appear, without duplicates.
///
/// Order is the template's rather than sorted, because the message that names them reads
/// alongside the value it came from and a reader is matching them off left to right.
pub fn slots(template: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        let name = &after[..close];
        // `{}` is not a slot and neither is `{ }`. A template carrying one is malformed
        // rather than parameterised, and inventing an empty-named binding for it would make
        // the refusal message unreadable.
        if !name.trim().is_empty() && !out.iter().any(|s| s == name) {
            out.push(name.to_string());
        }
        rest = &after[close + 1..];
    }
    out
}

/// Substitute every `{slot}` a binding names, and report what is left.
pub(crate) fn bind(template: &str, bindings: &[(String, String)]) -> Result<String, Vec<String>> {
    let mut filled = template.to_string();
    for (k, v) in bindings {
        filled = filled.replace(&format!("{{{k}}}"), v);
    }
    let remaining = slots(&filled);
    if remaining.is_empty() {
        Ok(filled)
    } else {
        Err(remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_named_in_order_and_deduplicated() {
        assert_eq!(
            slots("https://x/?sites={site}&p={param}&also={site}"),
            vec!["site", "param"]
        );
        assert_eq!(slots("https://x/no-slots"), Vec::<String>::new());
    }

    /// The lint check only asserts a `{` is present. `{}` satisfies that and binds nothing,
    /// so the planner must not treat it as a slot it could ask a caller to fill.
    #[test]
    fn an_empty_brace_is_not_a_slot() {
        assert_eq!(slots("https://x/{}"), Vec::<String>::new());
        assert_eq!(slots("https://x/{  }"), Vec::<String>::new());
    }

    #[test]
    fn an_unclosed_brace_ends_the_scan_rather_than_looping() {
        assert_eq!(slots("https://x/{site"), Vec::<String>::new());
        assert_eq!(slots("https://x/{a}/{b"), vec!["a"]);
    }
}
