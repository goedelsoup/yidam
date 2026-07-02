from __future__ import annotations

from dataclasses import dataclass


@dataclass
class TemplateMarker:
    instruction: str

    def kind_str(self) -> str:
        return "Template"


@dataclass
class RegenMarker:
    command: str
    content: str

    def kind_str(self) -> str:
        return "Regen"


Marker = TemplateMarker | RegenMarker


def parse_markers(text: str) -> list[Marker]:
    markers: list[Marker] = []
    lines = iter(text.splitlines())

    for line in lines:
        stripped = line.strip()

        if stripped.startswith("<!-- TEMPLATE:"):
            rest = stripped[len("<!-- TEMPLATE:"):]
            if rest.endswith("-->"):
                instruction = rest[:-3].strip()
                markers.append(TemplateMarker(instruction=instruction))
            continue

        if stripped.startswith("<!-- REGEN:"):
            rest = stripped[len("<!-- REGEN:"):]
            rest_stripped = rest.strip()
            if rest_stripped.endswith("-->"):
                command = rest_stripped[:-3].strip()
            else:
                command = rest.strip()
                for inner in lines:
                    t = inner.strip()
                    if t == "-->" or t.endswith("-->"):
                        break

            content_lines: list[str] = []
            for inner in lines:
                if inner.strip() == "<!-- /REGEN -->":
                    break
                content_lines.append(inner)
            content = "\n".join(content_lines).strip()
            markers.append(RegenMarker(command=command, content=content))

    return markers


def update_regen(text: str, command: str, new_content: str) -> str:
    open_tag = f"<!-- REGEN: {command}"
    close_tag = "<!-- /REGEN -->"

    open_pos = text.find(open_tag)
    if open_pos == -1:
        return text

    after_open = open_pos + len(open_tag)
    arrow_rel = text[after_open:].find("-->")
    if arrow_rel == -1:
        return text

    content_start = after_open + arrow_rel + 3
    close_rel = text[content_start:].find(close_tag)
    if close_rel == -1:
        return text

    close_abs = content_start + close_rel
    if new_content == "":
        # Clear the body without leaving a blank line between the markers.
        return f"{text[:content_start]}\n{text[close_abs:]}"
    return f"{text[:content_start]}\n{new_content}\n{text[close_abs:]}"
