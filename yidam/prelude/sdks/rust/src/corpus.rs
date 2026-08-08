use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EvidenceTag {
    Verified,
    Inference,
    Open,
    Implicit,
}

impl EvidenceTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidenceTag::Verified => "Verified",
            EvidenceTag::Inference => "Inference",
            EvidenceTag::Open => "Open",
            EvidenceTag::Implicit => "Implicit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Claim {
    pub text: String,
    pub tag: EvidenceTag,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Link {
    pub label: String,
    pub target: String,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodeKind {
    Concept,
    Decision,
    Authored,
    Generated,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Concept => "Concept",
            NodeKind::Decision => "Decision",
            NodeKind::Authored => "Authored",
            NodeKind::Generated => "Generated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CorpusNode {
    pub path: String,
    pub title: String,
    pub kind: NodeKind,
    pub claims: Vec<Claim>,
    pub links: Vec<Link>,
}

pub fn extract_links(text: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut in_link: Option<(String, String)> = None; // (dest, accumulated_label)
    let mut in_image = false;

    for event in Parser::new_ext(text, Options::empty()) {
        match event {
            Event::Start(Tag::Image { .. }) => {
                in_image = true;
            }
            Event::End(TagEnd::Image) => {
                in_image = false;
            }
            Event::Start(Tag::Link { dest_url, .. }) if !in_image => {
                in_link = Some((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Link) if !in_image => {
                if let Some((dest, label)) = in_link.take() {
                    let (target, anchor) = match dest.find('#') {
                        Some(pos) => {
                            let a = dest[pos + 1..].to_string();
                            (
                                dest[..pos].to_string(),
                                if a.is_empty() { None } else { Some(a) },
                            )
                        }
                        None => (dest, None),
                    };
                    links.push(Link {
                        label,
                        target,
                        anchor,
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if !in_image {
                    if let Some((_, ref mut label)) = in_link {
                        label.push_str(&t);
                    }
                }
            }
            _ => {}
        }
    }
    links
}

fn contains_md_link(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(2).any(|w| w == b"](")
}

pub fn extract_claims(text: &str) -> Vec<Claim> {
    let mut claims = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("<!--")
            || line.starts_with("```")
            || line.starts_with('|')
            || line.starts_with('-')
            || line.starts_with('*')
            || line.starts_with('!')
            || line.starts_with('>')
        {
            continue;
        }
        if let Some(t) = line.strip_suffix(" [verified]") {
            claims.push(Claim {
                text: t.to_string(),
                tag: EvidenceTag::Verified,
            });
        } else if let Some(t) = line.strip_suffix(" [inference]") {
            claims.push(Claim {
                text: t.to_string(),
                tag: EvidenceTag::Inference,
            });
        } else if let Some(t) = line.strip_suffix(" [open]") {
            claims.push(Claim {
                text: t.to_string(),
                tag: EvidenceTag::Open,
            });
        } else if !contains_md_link(line) {
            claims.push(Claim {
                text: line.to_string(),
                tag: EvidenceTag::Implicit,
            });
        }
    }
    claims
}

pub fn parse_node(path: &str, text: &str) -> CorpusNode {
    let title = text
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())
        .unwrap_or_default();

    let kind = if path.starts_with("corpus/") {
        NodeKind::Concept
    } else if path.starts_with("decisions/") {
        NodeKind::Decision
    } else {
        NodeKind::Authored
    };

    CorpusNode {
        path: path.to_string(),
        title,
        kind,
        claims: extract_claims(text),
        links: extract_links(text),
    }
}
