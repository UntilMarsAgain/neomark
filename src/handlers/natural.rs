//! 自然块展开器：把一段普通文字包进 `<p>`，并交给行内层解析。
//!
//! 块级结构（`<p>`）在这里决定，**行内结构**由 [`crate::inline`] 决定。

use crate::ast::{Ast, Attr, NodeId};
use crate::dispatch::{Context, Handler};
use crate::inline;

/// 自然块 → `<p class="nm-p">…</p>`，块体走行内层。
///
/// 多行文本按原样交给行内层：其中的裸换行是软换行（HTML 会折叠成空格），
/// 行尾反斜杠是硬换行（`<br>`）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphHandler;

impl Handler for ParagraphHandler {
    fn expand_natural(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(text) = ast.natural(node).map(|natural| natural.text.clone()) else {
            return Vec::new();
        };

        let paragraph = ast.new_element("p", vec![Attr::new("class", "nm-p")]);
        for child in inline::parse(ast, &text) {
            ast.append(paragraph, child);
        }

        vec![paragraph]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{Dispatcher, Registry};
    use crate::handlers;
    use crate::html;
    use crate::parse::parse;

    fn render(source: &str) -> String {
        let mut ast = parse(source);
        let mut registry = Registry::new();
        handlers::register_defaults(&mut registry);

        let mut ctx = Context::new(source);
        Dispatcher::new(registry).run(&mut ast, &mut ctx);
        html::render(&ast)
    }

    #[test]
    fn a_paragraph_wraps_its_inline_content() {
        assert_eq!(
            render("这是 **粗** 与 `代码`。"),
            "<p class=\"nm-p\">这是 <strong class=\"nm-strong\">粗</strong> 与 <code class=\"nm-code-inline\">代码</code>。</p>"
        );
    }

    #[test]
    fn a_multiline_paragraph_keeps_soft_breaks() {
        assert_eq!(
            render("第一行\n第二行"),
            "<p class=\"nm-p\">第一行\n第二行</p>"
        );
    }

    #[test]
    fn a_trailing_backslash_becomes_a_hard_break() {
        assert_eq!(
            render("第一行\\\n第二行"),
            "<p class=\"nm-p\">第一行<br>第二行</p>"
        );
    }
}
