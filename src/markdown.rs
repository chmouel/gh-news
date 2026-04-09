use crate::ui::theme::ColorPalette;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena};
use ratatui::prelude::*;

/// Strips HTML tags from a string, converting common elements to text formatting.
fn strip_html_tags(html: &str) -> String {
    // Convert common HTML elements to text equivalents
    let html = html
        // Line breaks
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        // Block elements with spacing
        .replace("</p>", "\n\n")
        .replace("</div>", "\n")
        .replace("</blockquote>", "\n")
        // Headers with visual distinction
        .replace("<h1>", "\n\n## ")
        .replace("</h1>", "\n")
        .replace("<h2>", "\n\n### ")
        .replace("</h2>", "\n")
        .replace("<h3>", "\n#### ")
        .replace("</h3>", "\n")
        .replace("<h4>", "\n##### ")
        .replace("</h4>", "\n")
        // Lists
        .replace("<li>", "  • ")
        .replace("</li>", "\n")
        .replace("</ul>", "\n")
        .replace("</ol>", "\n")
        // Code blocks
        .replace("<pre>", "\n```\n")
        .replace("</pre>", "\n```\n")
        .replace("<code>", "`")
        .replace("</code>", "`")
        // Tables
        .replace("</th>", " | ")
        .replace("</td>", " | ")
        .replace("</tr>", "\n")
        // Collapsible sections
        .replace("<details>", "\n")
        .replace("</details>", "\n")
        .replace("<summary>", "▶ ")
        .replace("</summary>", "\n");

    // Strip remaining tags
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Clean up excessive whitespace
    let mut cleaned = String::new();
    let mut newline_count = 0;
    for c in result.chars() {
        if c == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                cleaned.push(c);
            }
        } else {
            newline_count = 0;
            cleaned.push(c);
        }
    }
    cleaned
}

pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Render markdown to styled lines for Ratatui using comrak for proper parsing
    pub fn render_simple(markdown: &str, palette: &ColorPalette) -> Vec<Line<'static>> {
        // Configure comrak options
        let mut options = comrak::ComrakOptions::default();
        options.extension.strikethrough = true;
        options.extension.table = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.superscript = true;
        options.extension.header_ids = None;
        options.extension.footnotes = true;
        options.render.unsafe_ = true; // Allow raw HTML (for GitHub markdown)
        options.render.github_pre_lang = true;
        options.render.hardbreaks = true; // Treat soft breaks as hard breaks (line breaks)

        // Parse markdown into AST
        let arena = Arena::new();
        let root = parse_document(&arena, markdown, &options);

        let mut renderer = MarkdownToRatatui::new(palette);
        renderer.render(root)
    }
}

struct MarkdownToRatatui {
    lines: Vec<Line<'static>>,
    colors: ColorPalette,
    list_depth: usize,
    list_markers: Vec<String>,
    last_was_blank: bool, // Track if last line was blank to avoid duplicates
}

impl MarkdownToRatatui {
    fn new(palette: &ColorPalette) -> Self {
        Self {
            lines: Vec::new(),
            colors: palette.clone(),
            list_depth: 0,
            list_markers: Vec::new(),
            last_was_blank: false,
        }
    }

    fn add_blank_line(&mut self) {
        if !self.last_was_blank {
            self.lines.push(Line::from(vec![]));
            self.last_was_blank = true;
        }
    }

    fn add_line(&mut self, line: Line<'static>) {
        self.lines.push(line);
        self.last_was_blank = false;
    }

    fn render<'a>(&mut self, root: &'a AstNode<'a>) -> Vec<Line<'static>> {
        self.walk(root);
        self.lines.clone()
    }

    fn walk<'a>(&mut self, node: &'a AstNode<'a>) {
        match node.data.borrow().value {
            NodeValue::Document => {
                for child in node.children() {
                    self.walk(child);
                }
            }
            NodeValue::BlockQuote => {
                // Add blank line before blockquote
                if !self.last_was_blank && !self.lines.is_empty() {
                    self.add_blank_line();
                }

                for child in node.children() {
                    if matches!(child.data.borrow().value, NodeValue::Paragraph) {
                        let span_lines = self.render_inline_with_breaks(child);
                        for spans in span_lines {
                            let mut quote_spans =
                                vec![Span::styled("│ ", Style::default().fg(self.colors.fg_dim))];
                            quote_spans.extend(spans);
                            self.add_line(Line::from(quote_spans));
                        }
                    } else {
                        self.walk(child);
                    }
                }

                // Add blank line after blockquote
                self.add_blank_line();
            }
            NodeValue::List(list) => {
                // Add blank line before list if previous content wasn't blank
                if !self.last_was_blank && !self.lines.is_empty() {
                    self.add_blank_line();
                }

                self.list_depth += 1;
                let marker = if list.list_type == comrak::nodes::ListType::Bullet {
                    "•".to_string()
                } else {
                    "1.".to_string()
                };
                self.list_markers.push(marker);

                let items: Vec<_> = node.children().collect();
                for (idx, child) in items.iter().enumerate() {
                    self.walk_list_item(child, idx);
                    // Add blank line between list items
                    if idx < items.len() - 1 {
                        self.add_blank_line();
                    }
                }

                self.list_markers.pop();
                self.list_depth -= 1;
            }
            NodeValue::Item(_item) => {
                // This is handled in walk_list_item
            }
            NodeValue::DescriptionList => {
                for child in node.children() {
                    self.walk(child);
                }
            }
            NodeValue::DescriptionItem(_) => {
                for child in node.children() {
                    self.walk(child);
                }
            }
            NodeValue::DescriptionTerm => {
                let spans = self.render_inline(node);
                self.add_line(Line::from(spans));
            }
            NodeValue::DescriptionDetails => {
                for child in node.children() {
                    self.walk(child);
                }
            }
            NodeValue::CodeBlock(ref block) => {
                // Add blank line before code block
                if !self.last_was_blank && !self.lines.is_empty() {
                    self.add_blank_line();
                }

                let code = block.literal.as_str();
                let lang = block.info.as_str();

                // Render code block with box
                self.add_line(Line::from(vec![
                    Span::styled("┌─ ", Style::default().fg(self.colors.bg_dark)),
                    Span::styled(
                        lang.to_string(),
                        Style::default()
                            .fg(self.colors.blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default().fg(self.colors.bg_dark)),
                ]));

                for line in code.lines() {
                    self.add_line(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(self.colors.bg_dark)),
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(self.colors.fg).bg(self.colors.bg),
                        ),
                    ]));
                }

                self.add_line(Line::from(vec![Span::styled(
                    "└────────────────────────────────────────────────────────┘",
                    Style::default().fg(self.colors.bg_dark),
                )]));

                // Add blank line after code block
                self.add_blank_line();
            }
            NodeValue::HtmlBlock(ref block) => {
                let html = block.literal.as_str();
                let markdown = strip_html_tags(html);
                // Pass through comrak for proper styled rendering
                let rendered_lines = MarkdownRenderer::render_simple(&markdown, &self.colors);
                for line in rendered_lines {
                    self.lines.push(line);
                }
            }
            NodeValue::Paragraph => {
                let span_lines = self.render_inline_with_breaks(node);
                if span_lines.is_empty() {
                    // Empty paragraph - only add if we're not already blank
                    if !self.last_was_blank {
                        self.add_blank_line();
                    }
                } else {
                    for spans in span_lines {
                        self.add_line(Line::from(spans));
                    }
                }
            }
            NodeValue::Heading(ref heading) => {
                // Add blank line before heading if previous content wasn't blank
                if !self.last_was_blank && !self.lines.is_empty() {
                    self.add_blank_line();
                }

                let level = heading.level;
                let spans = self.render_inline(node);

                let color = match level {
                    1 => self.colors.red,
                    2 => self.colors.blue,
                    3 => self.colors.cyan,
                    _ => self.colors.fg,
                };

                // Style the header spans with bold and color
                let styled_spans: Vec<Span<'static>> = spans
                    .into_iter()
                    .map(|mut span| {
                        span.style = span.style.fg(color).add_modifier(Modifier::BOLD);
                        span
                    })
                    .collect();

                self.add_line(Line::from(styled_spans));

                // Add underline for h1 and h2 (like GitHub) - fixed 40 char width
                if level <= 2 {
                    let underline_char = if level == 1 { "═" } else { "─" };
                    self.add_line(Line::from(vec![Span::styled(
                        underline_char.repeat(40),
                        Style::default().fg(self.colors.fg_dim),
                    )]));
                }

                // Add blank line after heading
                self.add_blank_line();
            }
            NodeValue::ThematicBreak => {
                // Add blank line before thematic break
                if !self.last_was_blank && !self.lines.is_empty() {
                    self.add_blank_line();
                }
                self.add_line(Line::from(vec![Span::styled(
                    "────────────────────────────────────────────────────────",
                    Style::default().fg(self.colors.bg_dark),
                )]));
                // Add blank line after thematic break
                self.add_blank_line();
            }
            NodeValue::Table(_) => {
                self.render_table(node);
            }
            NodeValue::TableRow(_) => {
                // Handled in render_table
            }
            NodeValue::TableCell => {
                // Handled in render_table
            }
            NodeValue::FootnoteDefinition(_) => {
                // Skip footnote definitions for now
            }
            _ => {
                // For any other node types, try to render children
                for child in node.children() {
                    self.walk(child);
                }
            }
        }
    }

    fn walk_list_item<'a>(&mut self, item: &'a AstNode<'a>, _idx: usize) {
        let indent = "  ".repeat(self.list_depth.saturating_sub(1));
        let marker = self
            .list_markers
            .last()
            .cloned()
            .unwrap_or_else(|| "•".to_string());

        let mut first_paragraph = true;
        let children: Vec<_> = item.children().collect();

        for (para_idx, child) in children.iter().enumerate() {
            match child.data.borrow().value {
                NodeValue::Paragraph => {
                    let span_lines = self.render_inline_with_breaks(child);
                    for (line_idx, spans) in span_lines.iter().enumerate() {
                        let mut list_spans = vec![
                            Span::styled(indent.clone(), Style::default()),
                            Span::styled(
                                if first_paragraph && line_idx == 0 {
                                    format!("{} ", marker)
                                } else {
                                    "  ".to_string()
                                },
                                Style::default().fg(if marker == "•" {
                                    self.colors.green
                                } else {
                                    self.colors.blue
                                }),
                            ),
                        ];
                        list_spans.extend(spans.clone());
                        self.add_line(Line::from(list_spans));
                    }
                    first_paragraph = false;

                    // Add spacing between multiple paragraphs in the same list item
                    if para_idx < children.len() - 1 {
                        // Check if next child is also a paragraph
                        if let Some(next_child) = children.get(para_idx + 1) {
                            if matches!(next_child.data.borrow().value, NodeValue::Paragraph) {
                                self.add_blank_line();
                            }
                        }
                    }
                }
                NodeValue::List(_) => {
                    // Nested list - add a blank line before it if not first
                    if !first_paragraph {
                        self.add_blank_line();
                    }
                    self.walk(child);
                }
                _ => {
                    self.walk(child);
                }
            }
        }
    }

    fn render_table<'a>(&mut self, table: &'a AstNode<'a>) {
        let mut rows = Vec::new();
        let mut is_header = true;

        for row_node in table.children() {
            let mut cells = Vec::new();
            for cell_node in row_node.children() {
                let cell_text = self.render_inline(cell_node);
                let cell_str = cell_text
                    .iter()
                    .map(|s| s.content.as_ref().to_string())
                    .collect::<String>();
                cells.push(cell_str);
            }
            rows.push((cells, is_header));
            is_header = false;
        }

        if rows.is_empty() {
            return;
        }

        // Calculate column widths
        let num_cols = rows[0].0.len();
        let mut col_widths = vec![0; num_cols];

        for (cells, _) in &rows {
            for (i, cell) in cells.iter().enumerate() {
                col_widths[i] = col_widths[i].max(cell.len().min(30)); // Cap at 30 chars
            }
        }

        // Add blank line before table
        if !self.last_was_blank && !self.lines.is_empty() {
            self.add_blank_line();
        }

        // Render header separator
        let separator: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("─┬─");
        self.add_line(Line::from(vec![Span::styled(
            format!("┌─{}─┐", separator),
            Style::default().fg(self.colors.bg_dark),
        )]));

        // Render rows
        for (idx, (cells, is_header_row)) in rows.iter().enumerate() {
            let mut row_spans = vec![Span::styled("│ ", Style::default().fg(self.colors.bg_dark))];

            for (i, cell) in cells.iter().enumerate() {
                let padded = format!("{:<width$}", cell, width = col_widths[i]);
                let style = if *is_header_row {
                    Style::default()
                        .fg(self.colors.blue)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.colors.fg)
                };
                row_spans.push(Span::styled(padded, style));
                if i < cells.len() - 1 {
                    row_spans.push(Span::styled(
                        " │ ",
                        Style::default().fg(self.colors.bg_dark),
                    ));
                }
            }

            row_spans.push(Span::styled(" │", Style::default().fg(self.colors.bg_dark)));
            self.add_line(Line::from(row_spans));

            // Add separator after header
            if *is_header_row && idx < rows.len() - 1 {
                let separator: String = col_widths
                    .iter()
                    .map(|w| "─".repeat(*w))
                    .collect::<Vec<_>>()
                    .join("─┼─");
                self.add_line(Line::from(vec![Span::styled(
                    format!("├─{}─┤", separator),
                    Style::default().fg(self.colors.bg_dark),
                )]));
            }
        }

        // Render footer
        let separator: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("─┴─");
        self.add_line(Line::from(vec![Span::styled(
            format!("└─{}─┘", separator),
            Style::default().fg(self.colors.bg_dark),
        )]));

        // Add blank line after table
        self.add_blank_line();
    }

    /// Render inline content with support for line breaks - returns multiple lines as span vectors
    fn render_inline_with_breaks<'a>(&self, node: &'a AstNode<'a>) -> Vec<Vec<Span<'static>>> {
        let mut lines = Vec::new();
        let mut current_spans = Vec::new();

        for child in node.children() {
            match child.data.borrow().value {
                NodeValue::SoftBreak | NodeValue::LineBreak => {
                    // Save current line and start a new one
                    if !current_spans.is_empty() {
                        lines.push(current_spans);
                        current_spans = Vec::new();
                    } else {
                        // Empty line for explicit breaks
                        lines.push(Vec::new());
                    }
                }
                NodeValue::Text(ref text) => {
                    // Handle text that might contain newlines
                    let text_str = text.to_string();
                    if text_str.contains('\n') {
                        let parts: Vec<&str> = text_str.split('\n').collect();
                        for (i, part) in parts.iter().enumerate() {
                            if !part.is_empty() {
                                current_spans.push(Span::raw(part.to_string()));
                            }
                            // If not the last part, start a new line
                            if i < parts.len() - 1 {
                                if !current_spans.is_empty() {
                                    lines.push(current_spans);
                                    current_spans = Vec::new();
                                } else {
                                    lines.push(Vec::new());
                                }
                            }
                        }
                    } else {
                        current_spans.push(Span::raw(text_str));
                    }
                }
                _ => {
                    // Render the child and add to current spans
                    let child_spans = self.render_inline_single(child);
                    current_spans.extend(child_spans);
                }
            }
        }

        // Add any remaining spans as the last line
        if !current_spans.is_empty() {
            lines.push(current_spans);
        }

        lines
    }

    /// Render inline content as a single line (no breaks)
    fn render_inline<'a>(&self, node: &'a AstNode<'a>) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        for child in node.children() {
            // Skip soft breaks and line breaks in single-line mode
            if matches!(
                child.data.borrow().value,
                NodeValue::SoftBreak | NodeValue::LineBreak
            ) {
                continue;
            }
            let child_spans = self.render_inline_single(child);
            spans.extend(child_spans);
        }

        spans
    }

    /// Render a single inline node (recursive helper)
    fn render_inline_single<'a>(&self, node: &'a AstNode<'a>) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        match node.data.borrow().value {
            NodeValue::Text(ref text) => {
                // Text nodes might contain newlines - split them to preserve structure
                let text_str = text.to_string();
                if text_str.contains('\n') {
                    // Split on newlines and add each part
                    let parts: Vec<&str> = text_str.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if !part.is_empty() {
                            spans.push(Span::raw(part.to_string()));
                        }
                        // Add a soft break after each part except the last
                        if i < parts.len() - 1 {
                            // This will be handled by the parent's break detection
                        }
                    }
                } else {
                    spans.push(Span::raw(text_str));
                }
            }
            NodeValue::Code(ref code) => {
                let code_text = code.literal.as_str();
                // Inline code with background for better visibility (like GitHub)
                spans.push(Span::styled(
                    code_text.to_string(),
                    Style::default()
                        .fg(self.colors.yellow)
                        .bg(self.colors.bg_highlight)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            NodeValue::Emph => {
                let inner = self.render_inline(node);
                for mut span in inner {
                    span.style = span
                        .style
                        .fg(self.colors.cyan)
                        .add_modifier(Modifier::ITALIC);
                    spans.push(span);
                }
            }
            NodeValue::Strong => {
                let inner = self.render_inline(node);
                for mut span in inner {
                    span.style = span
                        .style
                        .fg(self.colors.magenta)
                        .add_modifier(Modifier::BOLD);
                    spans.push(span);
                }
            }
            NodeValue::Strikethrough => {
                let inner = self.render_inline(node);
                for mut span in inner {
                    span.style = span
                        .style
                        .fg(self.colors.fg_dim)
                        .add_modifier(Modifier::CROSSED_OUT);
                    spans.push(span);
                }
            }
            NodeValue::Link(ref link) => {
                let inner = self.render_inline(node);
                let url = link.url.as_str();
                let title = link.title.as_str();

                // Add link text
                for mut span in inner {
                    span.style = span
                        .style
                        .fg(self.colors.blue)
                        .add_modifier(Modifier::UNDERLINED);
                    spans.push(span);
                }

                // Add URL in parentheses if different from text
                if !url.is_empty() && !title.is_empty() {
                    spans.push(Span::styled(
                        format!(" ({})", url),
                        Style::default().fg(self.colors.fg_dim),
                    ));
                }
            }
            NodeValue::Image(ref image) => {
                // Get alt text from children
                let mut alt_text = String::new();
                for alt_child in node.children() {
                    if let NodeValue::Text(ref text) = alt_child.data.borrow().value {
                        alt_text.push_str(text);
                    }
                }
                let url = image.url.as_str();
                spans.push(Span::styled(
                    format!(
                        "[Image: {}]",
                        if alt_text.is_empty() {
                            "image"
                        } else {
                            &alt_text
                        }
                    ),
                    Style::default().fg(self.colors.cyan),
                ));
                if !url.is_empty() {
                    spans.push(Span::styled(
                        format!(" ({})", url),
                        Style::default().fg(self.colors.fg_dim),
                    ));
                }
            }
            NodeValue::HtmlInline(ref html) => {
                let text = strip_html_tags(html);
                if !text.trim().is_empty() {
                    spans.push(Span::raw(text));
                }
            }
            NodeValue::TaskItem(checked) => {
                let marker = if checked.is_some() { "☑" } else { "☐" };
                spans.push(Span::styled(
                    format!("{} ", marker),
                    Style::default().fg(self.colors.green),
                ));
                let inner = self.render_inline(node);
                spans.extend(inner);
            }
            _ => {
                // For other inline nodes, recursively render
                for child in node.children() {
                    spans.extend(self.render_inline_single(child));
                }
            }
        }

        spans
    }
}
