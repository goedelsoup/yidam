#[derive(Debug, Clone, PartialEq)]
pub enum Marker {
    Template { instruction: String },
    Regen { command: String, content: String },
}

impl Marker {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Marker::Template { .. } => "Template",
            Marker::Regen { .. } => "Regen",
        }
    }
}

pub fn parse_markers(text: &str) -> Vec<Marker> {
    let mut markers = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // Single-line TEMPLATE: <!-- TEMPLATE: instruction -->
        if let Some(rest) = trimmed.strip_prefix("<!-- TEMPLATE:") {
            if let Some(raw) = rest.strip_suffix("-->") {
                markers.push(Marker::Template { instruction: raw.trim().to_string() });
                continue;
            }
        }

        // REGEN: <!-- REGEN: command ... --> content <!-- /REGEN -->
        if let Some(rest) = trimmed.strip_prefix("<!-- REGEN:") {
            let command = if let Some(cmd) = rest.trim_end().strip_suffix("-->") {
                // single-line open tag: <!-- REGEN: cmd -->
                cmd.trim().to_string()
            } else {
                // multi-line: command is the rest of this line; inner lines end at -->
                let cmd = rest.trim().to_string();
                for inner in lines.by_ref() {
                    let t = inner.trim();
                    if t == "-->" || t.ends_with("-->") {
                        break;
                    }
                }
                cmd
            };

            let mut content_lines: Vec<&str> = Vec::new();
            for inner in lines.by_ref() {
                if inner.trim() == "<!-- /REGEN -->" {
                    break;
                }
                content_lines.push(inner);
            }
            let content = content_lines.join("\n").trim().to_string();
            markers.push(Marker::Regen { command, content });
        }
    }
    markers
}

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
    let content_start = after_open + arrow_rel + 3;
    let Some(close_rel) = text[content_start..].find(close_tag) else {
        return text.to_string();
    };
    let close_abs = content_start + close_rel;
    if new_content.is_empty() {
        // Clear the body without leaving a blank line between the markers.
        format!("{}\n{}", &text[..content_start], &text[close_abs..])
    } else {
        format!("{}\n{}\n{}", &text[..content_start], new_content, &text[close_abs..])
    }
}
