//! 展开器接口与展开上下文。

use crate::ast::{CallBlock, Node, Span};

/// 一个块展开器：把未展开的调用节点推进**一级**。
pub trait Handler {
    /// 消费未展开的调用节点，返回替换子树。
    ///
    /// 参数按值传入，展开器可以**移动** `call.body` 到替换子树里（零 clone），
    /// 也可以直接丢掉它——这正是「替换子树」能力的来源。
    ///
    /// 返回的子树里若仍含 [`Node::Call`]，调度器会继续展开。
    fn expand(&self, call: CallBlock, ctx: &mut Context<'_>) -> Node;
}

/// 展开过程中传给展开器的上下文。
pub struct Context<'a> {
    source: &'a str,
}

impl<'a> Context<'a> {
    /// 用原始输入构造上下文。
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// 原始输入。
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// 按位置区间从原始输入切片。
    ///
    /// 展开器想拿到某个块的**原样文本**（含头部行、含原始缩进）时用它；
    /// 只要块体本身就用 [`CallBlock::raw_body`]。
    pub fn slice(&self, span: Span) -> &'a str {
        span.slice(self.source)
    }
}
