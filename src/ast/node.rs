//! 展开/渲染树的节点。
//!
//! 这棵树在任一时刻可以**同时包含未展开节点与已展开节点**：调度器把
//! [`Node::Call`] / [`Node::Natural`] 交给注册的展开器，替换成其它变体；
//! 展开器返回的子树里若仍含未展开节点，调度器会继续推进，直到树中不再有。
//!
//! 关键在于：**未展开的子树保持语法形态**（[`Node::Call`] 内部仍是
//! `Vec<Block>`），它被替换成其它节点之前不会被访问。

use super::block::{Block, CallBlock, NaturalBlock};
use super::span::Span;

/// 展开/渲染树上的一个节点。
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// **未展开**的自然块。
    Natural(NaturalBlock),
    /// **未展开**的调用节点：内部 `body` 仍是语法块，尚未转换。
    Call(CallBlock),
    /// 已展开的 HTML 元素。
    Element(Element),
    /// 已展开的纯文本（**未转义**；转义是渲染器的事）。
    Text(String),
    /// 兄弟节点序列：一个块可以展开成多个节点。
    ///
    /// 这是展开器的**返回值通道**；调度器会把它摊平到父序列里，
    /// 因此最终的树中不会出现 `Fragment`。
    Fragment(Vec<Node>),
    /// 报错节点：某个展开器无法工作时的**最终回退**，是叶子。
    Error(ErrorNode),
}

impl Node {
    /// 把一个语法块转成节点：**只转一层**，未展开的块原样保留。
    pub fn seed(block: Block) -> Node {
        match block {
            Block::Natural(natural) => Node::Natural(natural),
            Block::Call(call) => Node::Call(call),
        }
    }

    /// 批量 [`Node::seed`]。
    pub fn seed_all(blocks: Vec<Block>) -> Vec<Node> {
        blocks.into_iter().map(Node::seed).collect()
    }

    /// 这个节点是否还没被展开。
    pub const fn is_unexpanded(&self) -> bool {
        matches!(self, Node::Natural(_) | Node::Call(_))
    }
}

/// 一个 HTML 元素。
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// 标签名，例如 `div`。
    pub tag: String,
    /// 属性。
    pub attrs: Vec<Attr>,
    /// 子节点。
    pub children: Vec<Node>,
}

/// 元素属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    /// 属性名。
    pub name: String,
    /// 属性值；`None` 表示**布尔属性**（`lazy` 而不是 `lazy="true"`）。
    pub value: Option<String>,
}

impl Attr {
    /// 普通属性。
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
        }
    }

    /// 布尔属性。
    pub fn boolean(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }

    /// 是否是布尔属性。
    pub const fn is_boolean(&self) -> bool {
        self.value.is_none()
    }
}

/// 报错节点：展开器无法工作时产出的**最终回退**。
///
/// 它是叶子——**底下不挂任何节点**。渲染器拿到它之后自行决定怎么显示，
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
