//! 报错节点：展开器无法工作时的最终回退。

use super::span::Span;

/// 报错节点。
///
/// 它是**叶子**——底下不挂任何节点。渲染器拿到它之后自行决定怎么显示，
/// 例如输出可见占位、HTML 注释，或者直接回显 [`ErrorNode::content`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorNode {
    /// 报错种类。
    pub kind: ErrorKind,
    /// 给人看的错误信息。
    pub message: String,
    /// 出错块在原文中的位置。
    pub span: Span,
    /// 未能展开的块在原文中的完整文本（含头部行、含原始缩进）。
    pub content: String,
}

impl ErrorNode {
    /// 构造一个报错节点。
    pub fn new(
        kind: ErrorKind,
        message: impl Into<String>,
        span: Span,
        content: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            span,
            content: content.into(),
        }
    }
}

/// 报错种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// 没有展开器认领这个块：调用名没注册，或者没有注册自然块展开器。
    NoHandler,
    /// 展开器自己判定无法展开这个块。
    ExpandFailed,
}
