//! 展开器接口与展开上下文。

use crate::ast::{Ast, ErrorKind, ErrorNode, KindTag, NodeId, Span};

/// 一个块展开器。
///
/// 展开器拿到的是**节点 id**（`Copy`），不是借用——这样「读载荷」和「改树」
/// 不会互相打架：每次访问都是短暂借用，全程不存在同时持有的可变借用。
///
/// 自然块和调用块走同一条分发路径：[`Handler::expand`] 按种类分发到
/// [`Handler::expand_call`] / [`Handler::expand_natural`]，后两者都有默认
/// 实现，默认行为是产出一个「没有展开器认领」的报错节点。
///
/// 返回值是**替换该节点的兄弟序列**：返回空表示整块丢弃，返回一个表示替换成
/// 一个节点，返回多个表示展开成多个兄弟节点。返回的节点里若仍含未展开的块，
/// 调度器会继续推进。
pub trait Handler {
    /// 展开一个未展开的块。默认按种类分发到下面两个方法。
    fn expand(&self, node: NodeId, ast: &mut Ast, ctx: &mut Context<'_>) -> Vec<NodeId> {
        match ast.tag(node) {
            Some(KindTag::Call) => self.expand_call(node, ast, ctx),
            Some(KindTag::Natural) => self.expand_natural(node, ast, ctx),
            _ => Vec::new(),
        }
    }

    /// 展开一个调用块。默认产出报错节点。
    fn expand_call(&self, node: NodeId, ast: &mut Ast, ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(call) = ast.call(node) else {
            return Vec::new();
        };
        let span = call.span;
        let name = call.name.clone();

        vec![ast.new_error(ErrorNode::new(
            ErrorKind::NoHandler,
            format!("未注册的调用块 ::{name}"),
            span,
            ctx.slice(span),
        ))]
    }

    /// 展开一个自然块。默认产出报错节点。
    fn expand_natural(&self, node: NodeId, ast: &mut Ast, ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(natural) = ast.natural(node) else {
            return Vec::new();
        };
        let span = natural.span;

        vec![ast.new_error(ErrorNode::new(
            ErrorKind::NoHandler,
            "没有注册自然块展开器",
            span,
            ctx.slice(span),
        ))]
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
    /// 只要块体本身就用 [`crate::ast::CallBlock::raw_body`]。
    pub fn slice(&self, span: Span) -> &'a str {
        span.slice(self.source)
    }
}
