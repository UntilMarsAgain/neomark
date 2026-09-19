//! 自然块展开器：把一段普通文字包进 `<p>`，并交给行内层解析。
//!
//! 块级结构（`<p>`）在这里决定，**行内结构**由 [`crate::inline`] 决定。

use crate::ast::{Ast, NodeId};
use crate::dispatch::{Context, Handler};
use crate::inline;

/// 自然块 → **段落**节点，块体走行内层。
///
/// 多行文本按原样交给行内层：其中的裸换行是软换行（HTML 会折叠成空格），
/// 行尾反斜杠是硬换行。
///
/// 这里只说「这是一个段落」，至于输出 `<p class="nm-p">` 还是别的，是
/// [`crate::html`] 的决定。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphHandler;

impl Handler for ParagraphHandler {
    fn expand_natural(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(natural) = ast.natural(node) else {
            return Vec::new();
        };
        let text = natural.text.clone();
        let span = natural.span;
        let inline_context = natural.inline;

        let inline = inline::parse(ast, &text, span);

        // 行内上下文（链接文本、行内调用的内容）里不能再套一层段落，
        // 否则会产出 <a><p>…</p></a> 这种非法结构。这个事实在**产出**时就
        // 记在块上了，不用看父节点猜。
        if inline_context {
            return inline;
        }

        let paragraph = ast.new_paragraph();
        for child in inline {
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
