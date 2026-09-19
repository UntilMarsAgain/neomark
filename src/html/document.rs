//! 完整 HTML 页面：渲染出的 DOM + 内置默认 CSS。

use super::render::{escape_text, render};
use crate::ast::Ast;

/// 内置默认样式表。
///
/// 单独放成 `.css` 文件（而不是 Rust 字符串），这样编辑器能高亮它、也方便
/// 直接改样式而不动代码。
pub const DEFAULT_CSS: &str = include_str!("default.css");

/// 把整棵树渲染成一个**自包含**的 HTML 页面。
///
/// 页面内嵌 [`DEFAULT_CSS`]，不依赖任何外部资源，双击就能看。
pub fn render_page(ast: &Ast, title: &str) -> String {
    let mut out = String::new();

    out.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");

    out.push_str("<title>");
    escape_text(title, &mut out);
    out.push_str("</title>\n");

    out.push_str("<style>\n");
    out.push_str(DEFAULT_CSS);
    out.push_str("</style>\n");

    out.push_str("</head>\n<body>\n<div class=\"nm-document\">\n");
    out.push_str(&render(ast));
    out.push_str("\n</div>\n</body>\n</html>\n");

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{Context, Dispatcher, Registry};
    use crate::handlers;
    use crate::parse::parse;

    fn page(source: &str, title: &str) -> String {
        let mut ast = parse(source);
        let mut registry = Registry::new();
        handlers::register_defaults(&mut registry);

        let mut ctx = Context::new(source);
        Dispatcher::new(registry).run(&mut ast, &mut ctx);

        render_page(&ast, title)
    }

    #[test]
    fn the_page_is_self_contained() {
        let html = page("你好", "标题");

        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.contains("<title>标题</title>"));
        // 默认 CSS 内嵌在页面里，没有外部依赖。
        assert!(html.contains("<style>"));
        assert!(html.contains(".nm-document"));
        assert!(!html.contains("<link "));
        assert!(html.ends_with("</html>\n"));
    }

    #[test]
    fn the_title_is_escaped() {
        let html = page("x", "a<b>&c");
        assert!(html.contains("<title>a&lt;b&gt;&amp;c</title>"));
    }

    #[test]
    fn the_body_only_contains_the_rendered_dom() {
        let html = page("一段话", "t");
        assert!(html.contains("<div class=\"nm-document\">\n<p class=\"nm-p\">一段话</p>\n</div>"));
    }
}
