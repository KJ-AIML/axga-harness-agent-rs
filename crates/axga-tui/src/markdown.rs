//! Minimal markdown renderer for ratatui.
//!
//! Converts markdown text into ratatui `Text` with styled spans.
//! Supports: **bold**, *italic*, `code`, ```code blocks```, - lists, # headings.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Theme tokens for markdown rendering.
pub struct MarkdownTheme {
    pub text: Color,
    pub code_bg: Color,
    pub code_fg: Color,
    pub heading: Color,
    pub bold: Color,
    pub link: Color,
    pub list_bullet: Color,
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self {
            text: Color::Rgb(245, 245, 245),
            code_bg: Color::Rgb(40, 40, 40),
            code_fg: Color::Rgb(0, 217, 255),
            heading: Color::Rgb(79, 168, 255),
            bold: Color::Rgb(255, 255, 255),
            link: Color::Rgb(79, 168, 255),
            list_bullet: Color::Rgb(158, 158, 158),
        }
    }
}

/// Render markdown text into ratatui `Text`.
pub fn render_markdown(md: &str, theme: &MarkdownTheme) -> Text<'static> {
    let lines: Vec<Line> = md.lines().map(|line| render_line(line, theme)).collect();
    Text::from(lines)
}

fn render_line(line: &str, theme: &MarkdownTheme) -> Line<'static> {
    let trimmed = line.trim();

    // Empty line
    if trimmed.is_empty() {
        return Line::from("");
    }

    // Code block (```)
    if trimmed.starts_with("```") {
        return Line::from(vec![Span::styled(
            trimmed.replacen("```", "", 1).trim().to_string(),
            Style::default().fg(theme.code_fg).bg(theme.code_bg),
        )]);
    }

    // Heading (#)
    if let Some(heading) = trimmed.strip_prefix("# ") {
        return Line::from(vec![Span::styled(
            heading.to_string(),
            Style::default().fg(theme.heading).add_modifier(Modifier::BOLD),
        )]);
    }
    if let Some(heading) = trimmed.strip_prefix("## ") {
        return Line::from(vec![Span::styled(
            heading.to_string(),
            Style::default().fg(theme.heading).add_modifier(Modifier::BOLD),
        )]);
    }

    // List item
    if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        let content = render_inline(item, theme);
        let mut spans = vec![Span::styled("  • ", Style::default().fg(theme.list_bullet))];
        spans.extend(content.spans);
        return Line::from(spans);
    }

    // Numbered list
    if let Some(item) = numbered_list_item(trimmed) {
        let content = render_inline(&item, theme);
        let num = trimmed.split('.').next().unwrap_or("1");
        let mut spans = vec![Span::styled(
            format!("  {num}. "),
            Style::default().fg(theme.list_bullet),
        )];
        spans.extend(content.spans);
        return Line::from(spans);
    }

    // Regular text with inline formatting
    render_inline(trimmed, theme)
}

fn numbered_list_item(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.splitn(2, ". ").collect();
    if parts.len() == 2 && parts[0].chars().all(|c| c.is_ascii_digit()) {
        Some(parts[1].to_string())
    } else {
        None
    }
}

fn render_inline(text: &str, theme: &MarkdownTheme) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();
    let mut remaining = text.to_string();
    let default_style = Style::default().fg(theme.text);

    while !remaining.is_empty() {
        // Find the earliest marker
        let bold = remaining.find("**");
        let italic = remaining.find('*');
        let code = remaining.find('`');
        let link = remaining.find('[');

        let pos = [bold, italic, code, link]
            .iter()
            .filter_map(|&p| p)
            .min();

        match pos {
            None => {
                spans.push(Span::styled(remaining.clone(), default_style));
                break;
            }
            Some(p) => {
                // Text before marker
                if p > 0 {
                    spans.push(Span::styled(remaining[..p].to_string(), default_style));
                }

                if bold == Some(p) && p < remaining.len() - 1 {
                    // **bold**
                    if let Some(end) = remaining[p + 2..].find("**") {
                        let bold_text = &remaining[p + 2..p + 2 + end];
                        spans.push(Span::styled(
                            bold_text.to_string(),
                            default_style.fg(theme.bold).add_modifier(Modifier::BOLD),
                        ));
                        remaining = remaining[p + 4 + end..].to_string();
                    } else {
                        spans.push(Span::styled("**".to_string(), default_style));
                        remaining = remaining[p + 2..].to_string();
                    }
                } else if italic == Some(p) && p < remaining.len() - 1 {
                    // *italic* (but not **)
                    if remaining.chars().nth(p + 1) != Some('*') {
                        if let Some(end) = remaining[p + 1..].find('*') {
                            let italic_text = &remaining[p + 1..p + 1 + end];
                            spans.push(Span::styled(
                                italic_text.to_string(),
                                default_style.add_modifier(Modifier::ITALIC),
                            ));
                            remaining = remaining[p + 2 + end..].to_string();
                        } else {
                            spans.push(Span::styled("*".to_string(), default_style));
                            remaining = remaining[p + 1..].to_string();
                        }
                    } else {
                        spans.push(Span::styled(remaining[p..p + 1].to_string(), default_style));
                        remaining = remaining[p + 1..].to_string();
                    }
                } else if code == Some(p) {
                    // `inline code`
                    if let Some(end) = remaining[p + 1..].find('`') {
                        let code_text = &remaining[p + 1..p + 1 + end];
                        spans.push(Span::styled(
                            code_text.to_string(),
                            Style::default().fg(theme.code_fg).bg(theme.code_bg),
                        ));
                        remaining = remaining[p + 2 + end..].to_string();
                    } else {
                        spans.push(Span::styled("`".to_string(), default_style));
                        remaining = remaining[p + 1..].to_string();
                    }
                } else if link == Some(p) {
                    // [text](url) — render as styled link text
                    if let Some(bracket_end) = remaining[p + 1..].find("](") {
                        let link_text = &remaining[p + 1..p + 1 + bracket_end];
                        let after_bracket = p + 1 + bracket_end + 2; // after "]("
                        if let Some(paren_end) = remaining[after_bracket..].find(')') {
                            spans.push(Span::styled(
                                link_text.to_string(),
                                Style::default()
                                    .fg(theme.link)
                                    .add_modifier(Modifier::UNDERLINED),
                            ));
                            remaining = remaining[after_bracket + paren_end + 1..].to_string();
                        } else {
                            spans.push(Span::styled(remaining[p..].to_string(), default_style));
                            break;
                        }
                    } else {
                        spans.push(Span::styled("[".to_string(), default_style));
                        remaining = remaining[p + 1..].to_string();
                    }
                } else {
                    spans.push(Span::styled(remaining[p..p + 1].to_string(), default_style));
                    remaining = remaining[p + 1..].to_string();
                }
            }
        }
    }

    Line::from(spans)
}
