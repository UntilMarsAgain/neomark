//! 自然块展开器：把一段普通文字原样放进 `<p>`。
//!
//! 目前**没有任何行内语法**——文字原样成为文本节点，转义由渲染器负责。

use crate::ast::{Ast, Attr, NodeId};
use crate::dispatch::{Context, Handler};

/// 自然块 → `<p class="nm-p">原文</p>`。
///
/// 多行文本不去动它：HTML 会把换行折叠成空格，这正是段落该有的行为。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphHandler;

impl Handler for ParagraphHandler {
    fn expand_natural(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
        let text = match ast.natural(node) {
            Some(natural) => natural.text.clone(),
            None => return Vec::new(),
        };

        let paragraph = ast.new_element("p", vec![Attr::new("class", "nm-p")]);
        let content = ast.new_text(text);
        ast.append(paragraph, content);

        vec![paragraph]
    }
}
