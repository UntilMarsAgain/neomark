//! 块的自身数据。
//!
//! 注意：这两类块都**只有一个块自己的信息**，没有任何孩子字段——块体与
//! 子节点由 [`crate::ast::Ast`] 的 arena 边表示。

use super::params::Params;
use super::span::Span;

/// 自然块：被空行（或调用块行首）切分出来的一段普通文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalBlock {
    /// 块的原文，多行用 `\n` 连接，不含首尾空行与行尾换行。
    pub text: String,
    /// 位置范围。
    pub span: Span,
}

impl NaturalBlock {
    /// 块的原文。
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// 调用块：`::name 参数...:` 的头部信息。
///
/// 块体不在这里——它是这个节点在 arena 里的**子节点**。
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
