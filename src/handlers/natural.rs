//! 自然块展开器：把一段文本按自然块规则解析。
//!
//! 它同时是两样东西：
//!
//! * 一个 [`Handler`]——调度器按自然块槽位调用它；
//! * 一个**可直接调用的接口**——其他展开器（`::h1` 的标题正文、`::quote` 的
//!   引用正文）或外部解析器需要「把这段文本当自然块解析」时，直接调
//!   [`NaturalExpander::block`] / [`NaturalExpander::inline`] 即可，
//!   不必绕调度器。
//!
//! 它持有行内层的[配置](Options)，所以「这一段用哪几项行内语法」是可以逐个
//! 注册表定制的。

use crate::ast::{Ast, NodeId, Span};
use crate::dispatch::{Context, Handler};
use crate::inline::{self, Options};

/// 自然块展开器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalExpander {
    options: Options,
}

impl Default for NaturalExpander {
    fn default() -> Self {
        Self::new(Options::default())
    }
}

impl NaturalExpander {
    /// 用给定的行内配置构造。
    pub const fn new(options: Options) -> Self {
        Self { options }
    }

    /// 当前的行内配置。
    pub const fn options(&self) -> Options {
        self.options
    }

    /// 把一段文本展开成**块级**内容：行内层的结果包在一个段落里。
    ///
    /// 可以直接调用：外部解析器判断出「这里是一段正文」时用它。
    pub fn block(&self, ast: &mut Ast, text: &str, span: Span) -> Vec<NodeId> {
        let inline = self.inline(ast, text, span);

        let paragraph = ast.new_paragraph();
        for child in inline {
            ast.append(paragraph, child);
        }

        vec![paragraph]
    }

    /// 把一段文本展开成**行内**内容：不套段落。
    ///
    /// 链接文本、行内调用的内容、`::h1` 的标题正文都走这一条。直接调用它也是
    /// 「把参数值当行内内容解析」的正路。
    pub fn inline(&self, ast: &mut Ast, text: &str, span: Span) -> Vec<NodeId> {
        inline::parse(ast, text, span, self.options)
    }
}

impl Handler for NaturalExpander {
    fn expand_natural(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(natural) = ast.natural(node) else {
            return Vec::new();
        };
        let text = natural.text.clone();
        let span = natural.span;
        let inline_context = natural.inline;

        // 行内上下文（链接文本、行内调用的内容）里不能再套一层段落，否则会产出
        // `<a><p>…</p></a>` 这种非法结构。这个事实在**产出**时就记在块上了，
        // 不用看父节点猜。
        if inline_context {
            return self.inline(ast, &text, span);
        }

        self.block(ast, &text, span)
    }
}
