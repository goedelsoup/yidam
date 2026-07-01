use anyhow::{Context, Result};
use std::path::Path;

// Replaces content between <!-- REGEN: <command> ... --> and <!-- /REGEN -->.
// Returns the original string unchanged if either marker is not found.
pub fn update_regen(text: &str, command: &str, new_content: &str) -> String {
    let open_tag = format!("<!-- REGEN: {command}");
    let close_tag = "<!-- /REGEN -->";

    let Some(open_pos) = text.find(&open_tag) else {
        return text.to_string();
    };
    let after_open = open_pos + open_tag.len();
    let Some(arrow_rel) = text[after_open..].find("-->") else {
        return text.to_string();
    };
    let content_start = after_open + arrow_rel + 3; // right after "-->"
    let Some(close_rel) = text[content_start..].find(close_tag) else {
        return text.to_string();
    };
    let close_abs = content_start + close_rel;
    format!(
        "{}\n{}\n{}",
        &text[..content_start],
        new_content,
        &text[close_abs..]
    )
}

pub fn update_file_regen(path: &Path, command: &str, new_content: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let updated = update_regen(&original, command, new_content);
    if updated != original {
        std::fs::write(path, &updated).with_context(|| format!("writing {}", path.display()))?;
        println!("  updated {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_regen_basic() {
        let input = "\
## Status\n\
\n\
<!-- REGEN: yidam status\n\
Fields: node count, open questions.\n\
-->\n\
_Run `yidam status` to populate._\n\
<!-- /REGEN -->\n";
        let expected = "\
## Status\n\
\n\
<!-- REGEN: yidam status\n\
Fields: node count, open questions.\n\
-->\n\
**12 nodes** · 3 open · index fresh\n\
<!-- /REGEN -->\n";
        assert_eq!(
            update_regen(input, "yidam status", "**12 nodes** · 3 open · index fresh"),
            expected
        );
    }

    #[test]
    fn missing_marker_is_noop() {
        let input = "# No REGEN here\n";
        assert_eq!(update_regen(input, "yidam status", "new content"), input);
    }

    #[test]
    fn idempotent() {
        let input = "<!-- REGEN: yidam status\n-->\ncontent\n<!-- /REGEN -->\n";
        let once = update_regen(input, "yidam status", "content");
        let twice = update_regen(&once, "yidam status", "content");
        assert_eq!(once, twice);
    }
}
