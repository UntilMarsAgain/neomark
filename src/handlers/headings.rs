//! 标题块：`::h1` ~ `::h6`。
//!
//! 用**一条通配模式** `h?` 一次注册，展开器再从调用名里读出级别——这正是通配
//! 分发要解决的场景：`h1`~`h6` 是同一个展开器的六个名字。
//!
//! 标题正文走的是自然块展开器的**可调用接口**（[`NaturalExpander::inline`]），
//! 因为标题里要的是行内内容，不是段落。

use crate::ast::{Ast, ErrorKind, ErrorNode, NodeId, NodeKind};
use crate::dispatch::{Context, Handler};

use super::natural::NaturalExpander;

/// 标题展开器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headings {
    natural: NaturalExpander,
}

impl Default for Headings {
    fn default() -> Self {
        Self::new(NaturalExpander::default())
    }
}

impl Headings {
    /// 用给定的自然块展开器构造（它负责解析标题正文里的行内标记）。
    pub const fn new(natural: NaturalExpander) -> Self {
        Self { natural }
    }

    /// 从调用名里读级别：只有 `h1` ~ `h6` 算数。
    ///
    /// 默认注册用的模式 `^h[1-6]$` 已经挡过一次了；这里再挡一次，是为了让
    /// [`Headings`] 被挂到更宽的模式上时也不会产出非法级别。
    fn level(name: &str) -> Option<u8> {
        let level: u8 = name.strip_prefix('h')?.parse().ok()?;
        (1..=6).contains(&level).then_some(level)
    }
}

impl Handler for Headings {
    fn expand_call(&self, node: NodeId, ast: &mut Ast, ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(call) = ast.call(node) else {
            return Vec::new();
        };
        let name = call.name.clone();
        let span = call.span;
        let body = call.raw_body.clone();

        let Some(level) = Self::level(&name) else {
            // `h?` 也会命中 `ha` 这种名字，报错比静默丢掉好。
            return vec![ast.new_error(ErrorNode::new(
                ErrorKind::ExpandFailed,
                format!("标题级别只能是 h1~h6，给的是 ::{name}"),
                span,
                ctx.slice(span),
            ))];
        };

        let heading = ast.new_node(NodeKind::Heading { level });
        for child in self.natural.inline(ast, &body, span) {
            ast.append(heading, child);
        }

        vec![heading]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_h1_to_h6_are_heading_names() {
        assert_eq!(Headings::level("h1"), Some(1));
        assert_eq!(Headings::level("h6"), Some(6));
        assert_eq!(Headings::level("h0"), None);
        assert_eq!(Headings::level("h7"), None);
        assert_eq!(Headings::level("ha"), None);
        assert_eq!(Headings::level("h"), None);
        assert_eq!(Headings::level("h10"), None);
    }
}
