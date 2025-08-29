//! Rust-side Markdown → HTML renderer with code highlighting and sanitization.
//! Feature-gated with `ui_render_rust`.
//! - Markdown: comrak (GFM)
//! - Highlight: syntect (classed output)
//! - Sanitize: ammonia (allow class attributes)

use ammonia::Builder as AmmoniaBuilder;
use comrak::{parse_document, Arena, ComrakOptions, format_html};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::html::ClassedHTMLGenerator;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum SyntaxTheme {
    Light,
    Dark,
    Auto,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub theme: SyntaxTheme,
    pub allow_html: bool,
}

/// Render markdown to sanitized HTML, with classed code highlighting.
pub fn render_markdown_to_safe_html(md: &str) -> String {
    render_markdown_to_safe_html_with(md, &RenderOptions { theme: SyntaxTheme::Auto, allow_html: false })
}

#[allow(dead_code)]
pub fn render_markdown_to_safe_html_with(md: &str, _opts: &RenderOptions) -> String {
    // Comrak options (GFM-like)
    let mut opt = ComrakOptions::default();
    opt.extension.strikethrough = true;
    opt.extension.table = true;
    opt.extension.autolink = true;
    opt.extension.tasklist = true;
    opt.extension.superscript = true;
    opt.render.github_pre_lang = true;

    // Parse AST
    let arena = Arena::new();
    let root = parse_document(&arena, md, &opt);

    // Prepare syntect resources (currently not rewriting AST)

    // Note: For maximum determinism and simplicity, we currently do not rewrite AST code blocks.
    // Comrak with github_pre_lang emits <code class="language-xxx"> which our CSS can style.

    // Render HTML from AST
    let mut html = Vec::new();
    format_html(root, &opt, &mut html).ok();
    let html = String::from_utf8(html).unwrap_or_default();

    // Sanitization: allow classes for language-* and syntect-*
    let mut builder = AmmoniaBuilder::default();
    builder.add_allowed_classes("code", std::iter::once("language-*"));
    builder.add_allowed_classes("span", std::iter::once("syntect-*")).add_tags(std::iter::once("span"));
    builder.add_tags(["pre", "code", "span", "table", "thead", "tbody", "tr", "th", "td", "a", "blockquote", "ul", "ol", "li"]);
    builder.url_relative(ammonia::UrlRelative::Deny);
    builder.add_generic_attributes(std::iter::once("class"));
    builder.add_tag_attribute_values("code", "class", std::iter::once("language-*"));
    builder.clean(&html).to_string()
}

/// Highlight a code string with language hint into classed HTML fragment (without surrounding <pre><code>).
/// Currently unused by default path; provided for future AST-integrated highlighting.
#[allow(dead_code)]
pub fn highlight_code_classed(code: &str, language: Option<&str>) -> String {
    let ps: SyntaxSet = SyntaxSet::load_defaults_newlines();
    let _ts: ThemeSet = ThemeSet::load_defaults();
    let syntax = language
        .and_then(|l| ps.find_syntax_by_token(l))
        .unwrap_or_else(|| ps.find_syntax_plain_text());
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, &ps, syntect::html::ClassStyle::Spaced);
    for line in code.lines() { let _ = generator.parse_html_for_line_which_includes_newline(line); }
    generator.finalize()
}
