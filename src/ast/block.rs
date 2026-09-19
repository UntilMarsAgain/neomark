//! 块的自身数据。
//!
//! 注意：这里的块都**只有一个块自己的信息**，没有任何孩子字段——块体与
//! 子节点由 [`crate::ast::Ast`] 的 arena 边表示。

use super::params::Params;
use super::span::Span;

/// 一个**尚未解析（展开）**的块。
///
/// 这是语法层交给分发器的东西。渲染树只区分「展开了没有」，不区分语法
/// 分类——所以 [`crate::ast::NodeKind`] 里只有一个 `Unparsed(Block)`，
/// 而不为每一类语法块各开一个变体。语法层将来加第三类块时，渲染器与
/// `NodeKind` 都不用动。
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// 自然块。没有名字，走专门的槽位而非按名查找。
    Natural(NaturalBlock),
    /// 调用块。按 `name` 查找展开器。
    Call(CallBlock),
}

impl Block {
    /// 块在原文中的位置。
    pub const fn span(&self) -> Span {
        match self {
            Block::Natural(natural) => natural.span,
            Block::Call(call) => call.span,
        }
    }
}

/// 自然块：被空行（或调用块行首）切分出来的一段普通文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalBlock {
    /// 块的原文，多行用 `\n` 连接，不含首尾空行与行尾换行。
    pub text: String,
    /// 位置范围。
    pub span: Span,
    /// 这段文本是**行内内容**（链接文本、行内调用的内容）还是块级内容。
    ///
    /// 由**产出方**决定：切分层产出的是块级，行内层产出的是行内。展开器据此
    /// 决定要不要给它套一层段落——这样就不必看父节点猜上下文，而上下文本就是
    /// 产出时就知道的事实。
    pub inline: bool,
}

impl NaturalBlock {
    /// 块的原文。
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// 调用块：`::name 参数...:` 的头部信息。
///
/// 块体不在这里——它是这个块在 arena 里的**子节点**。
#[derive(Debug, Clone, PartialEq)]
pub struct CallBlock {
    /// 调用名，例如 `notice`。
    pub name: String,
    /// 参数表（键与值都是字符串）。
    pub params: Params,
    /// 块体去掉公共缩进后的原文，多行用 `\n` 连接。
    ///
    /// 递归解析不可逆，所以需要原样文本的展开器（`::code` 之类）用它，
    /// 而不是从子节点反推。没有块体时为空字符串。
    pub raw_body: String,
    /// 位置范围：从头部行到块体最后一行（没有块体时就是头部行）。
    pub span: Span,
}

impl CallBlock {
    /// 调用名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 取某个字符串参数。
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key)
    }

    /// 块体去缩进后的原文。
    pub fn raw_body(&self) -> &str {
        &self.raw_body
    }
}
