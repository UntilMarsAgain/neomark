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
    use crate::parse::parse;

    fn page(source: &str, title: &str) -> String {
        let mut ast = parse(source);
        let mut registry = Registry::new();
        crate::stdlib::register_defaults(&mut registry);

        let mut ctx = Context::new(source);
        Dispatcher::new(registry).run(&mut ast, &mut ctx);

        render_page(&ast, title)
    }

    #[test]
    fn the_default_css_styles_every_class_our_blocks_emit() {
        // 类名写在渲染器里、样式写在 default.css 里，两边分处两地：
        // 改了名字忘了改另一边，页面上不会报错，只会没样式。
        for class in [
            ".nm-document",
            ".nm-p",
            ".nm-h1",
            ".nm-h6",
            ".nm-quote",
            ".nm-quote-origin",
            ".nm-code-block",
            ".nm-code-inline",
            ".nm-instance",
            ".nm-error",
            ".nm-error-inline",
            ".nm-icon",
            ".nm-link",
            ".nm-mark",
            ".nm-math",
        ] {
            assert!(DEFAULT_CSS.contains(class), "默认样式表里缺少 {class}");
        }
    }

    #[test]
    fn padded_boxes_control_their_own_inner_spacing() {
        // `.nm-p` 自带 1.1em 下外边距。带 padding 的框如果只清首尾子元素，
        // 一旦末尾多出别的孩子（比如引文的出处），那个下外边距就会漏出来，
        // 还会和下一个兄弟的上外边距折叠成较大者。所以框必须自己接管框内间距。
        assert!(DEFAULT_CSS.contains(".nm-quote > * + *"));
        assert!(DEFAULT_CSS.contains(".nm-quote > .nm-quote-origin"));
        assert!(DEFAULT_CSS.contains(".nm-document > :first-child"));
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
