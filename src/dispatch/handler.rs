//! 展开器接口与展开上下文。

use crate::ast::{Block, CallBlock, ErrorKind, ErrorNode, NaturalBlock, Node, Span};

/// 一个块展开器。
///
/// 自然块和调用块都是「未展开的块」，走同一条分发路径：[`Handler::expand`]
/// 按块的两类分发到 [`Handler::expand_call`] / [`Handler::expand_natural`]。
/// 后两者都有默认实现，默认行为是产出一个「没有展开器认领」的报错节点。
///
/// 于是：
///
/// * 只关心调用块的展开器，只实现 [`Handler::expand_call`]；
/// * 只关心自然块的展开器，只实现 [`Handler::expand_natural`]；
/// * 兜底展开器 [`Fallback`] 一个方法都不用实现。
pub trait Handler {
    /// 展开一个未展开的块。默认按块的两类分发到下面两个方法。
    fn expand(&self, block: Block, ctx: &mut Context<'_>) -> Node {
        match block {
            Block::Call(call) => self.expand_call(call, ctx),
            Block::Natural(natural) => self.expand_natural(natural, ctx),
        }
    }

    /// 展开一个调用块。默认产出报错节点。
    fn expand_call(&self, call: CallBlock, ctx: &mut Context<'_>) -> Node {
        Node::Error(ErrorNode::new(
            ErrorKind::NoHandler,
            format!("未注册的调用块 ::{}", call.name),
            call.span,
            ctx.slice(call.span),
        ))
    }

    /// 展开一个自然块。默认产出报错节点。
    fn expand_natural(&self, natural: NaturalBlock, ctx: &mut Context<'_>) -> Node {
        Node::Error(ErrorNode::new(
            ErrorKind::NoHandler,
            "没有注册自然块展开器",
            natural.span,
            ctx.slice(natural.span),
        ))
    }
}

/// 兜底展开器：没有展开器认领这个块时使用。
///
/// 它**就是 [`Handler`] 的全部默认行为**——把块的原文内容原样放进报错节点，
/// 再由渲染器决定怎么显示。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Fallback;

impl Handler for Fallback {}

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
