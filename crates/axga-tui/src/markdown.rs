//! Minimal markdown renderer for ratatui.
//!
//! Converts markdown text into ratatui `Text` with styled spans.
//! Supports: **bold**, *italic*, `code`, ```fenced code blocks``` (with
//! syntax highlighting via syntect), - lists, # headings, [links](url).

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxSet, SyntaxReference};


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

// ── Lazy syntect resources ────────────────────────────────────────────

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn highlight_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        // base16-ocean.dark looks good on most dark terminal backgrounds
        ts.themes["base16-ocean.dark"].clone()
    })
}

/// Convert a syntect `HighlightStyle` into a ratatui `Style`.
fn to_ratatui_style(s: syntect::highlighting::Style) -> Style {
    let fg = Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b);
    let mut style = Style::default().fg(fg);
    if s.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

// ── Public API ────────────────────────────────────────────────────────

/// Render markdown text into ratatui `Text`.
pub fn render_markdown(md: &str, theme: &MarkdownTheme) -> Text<'static> {
    let mut lines: Vec<Line> = Vec::new();
    let mut iter = md.lines().peekable();

    while let Some(line) = iter.next() {
        let trimmed = line.trim();

        // Fenced code block opener: ```lang  or  ~~~~lang
        if trimmed.starts_with("```") || trimmed.starts_with("~~~~") {
            let fence_char = &trimmed[..1]; // ` or ~
            let fence = if fence_char == "`" { "```" } else { "~~~~" };
            let lang = trimmed
                .strip_prefix(fence)
                .unwrap_or("")
                .trim()
                .to_string();

            // Collect lines until closing fence
            let mut code_lines: Vec<&str> = Vec::new();
            for inner in iter.by_ref() {
                if inner.trim() == fence {
                    break; // closing fence
                }
                code_lines.push(inner);
            }

            lines.extend(render_code_block(&code_lines, &lang, theme));
        } else {
            lines.push(render_line(line, theme));
        }
    }

    Text::from(lines)
}

// ── Diff rendering ────────────────────────────────────────────────────

/// Render a unified-diff snippet.  Lines starting with `+` get
/// `added_color`, lines starting with `-` get `removed_color`,
/// everything else gets `neutral_color`.
pub fn render_diff(
    text: &str,
    added_color: Color,
    removed_color: Color,
    neutral_color: Color,
) -> Text<'static> {
    let lines: Vec<Line> = text
        .lines()
        .map(|line| {
            if line.starts_with('+') {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(added_color),
                ))
            } else if line.starts_with('-') {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(removed_color),
                ))
            } else {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(neutral_color),
                ))
            }
        })
        .collect();
    Text::from(lines)
}

// ── Code block rendering ──────────────────────────────────────────────

/// Highlight a fenced code block using syntect.  Falls back to plain
/// styled text when the language is not recognised.
fn render_code_block(
    code_lines: &[&str],
    lang: &str,
    theme: &MarkdownTheme,
) -> Vec<Line<'static>> {
    let syntax = find_syntax(lang);
    let plain_style = Style::default().fg(theme.code_fg).bg(theme.code_bg);

    let mut out: Vec<Line<'static>> = Vec::with_capacity(code_lines.len() + 1);
    // Leading border line
    out.push(Line::from(Span::styled(
        format!("┌─ {} ─", if lang.is_empty() { "code" } else { lang }),
        Style::default().fg(theme.list_bullet),
    )));

    if code_lines.is_empty() {
        // Closing border
        out.push(Line::from(Span::styled(
            "└─".to_string(),
            Style::default().fg(theme.list_bullet),
        )));
        return out;
    }

    if let Some(syntax) = syntax {
        let mut highlighter =
            syntect::easy::HighlightLines::new(syntax, highlight_theme());
        for &line in code_lines {
            let styled = highlighter
                .highlight_line(line, syntax_set())
                .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);
            let spans: Vec<Span> = styled
                .into_iter()
                .map(|(style, text)| {
                    Span::styled(text.to_string(), to_ratatui_style(style))
                })
                .collect();
            out.push(Line::from(spans));
        }
    } else {
        for &line in code_lines {
            out.push(Line::from(Span::styled(
                line.to_string(),
                plain_style,
            )));
        }
    }

    // Closing border
    out.push(Line::from(Span::styled(
        "└─".to_string(),
        Style::default().fg(theme.list_bullet),
    )));
    out
}

/// Resolve a language identifier to a syntect `SyntaxReference`.  Common
/// aliases (e.g. `rs` → Rust, `py` → Python, `js` → JavaScript) are
/// handled by syntect's built-in extension/name matching.
fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let lang = lang.trim().to_lowercase();
    if lang.is_empty() {
        return None;
    }
    syntax_set()
        .find_syntax_by_token(&lang)
        .or_else(|| syntax_set().find_syntax_by_extension(&lang))
}

// ── Single-line rendering (unchanged) ─────────────────────────────────

fn render_line(line: &str, theme: &MarkdownTheme) -> Line<'static> {
    let trimmed = line.trim();

    // Empty line
    if trimmed.is_empty() {
        return Line::from("");
    }

    // Code block delimiter (stray fence — should not normally reach here,
    // but keep as safe fallback)
    if trimmed.starts_with("```") || trimmed.starts_with("~~~~") {
        return Line::from("");
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
