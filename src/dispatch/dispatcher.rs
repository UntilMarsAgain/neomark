//! 调度器：把未展开的块推进到不动点。

use super::handler::{Context, Handler};
use super::registry::Registry;
use crate::ast::{Block, Node};

/// 反复展开未展开的块，直到树中不再有 [`Node::Call`] / [`Node::Natural`]。
///
/// # 分发
///
/// 调用块按调用名查找展开器，自然块查找自然块展开器；**两者都没命中时用
/// 兜底展开器**——默认的 [`Fallback`](super::Fallback) 会把块的原文内容
/// 放进报错节点，因此任何块都不会被静默丢弃。
///
/// # 展开顺序
///
/// 自上而下：先展开当前未展开节点，再递归处理**替换结果**的子节点。
/// 因此展开器对自己子节点是否被访问有完全的控制权。
///
/// # 终止性
///
/// 改写规则不保证收敛——如果某个展开器展开出的子树里又出现同一个块，
/// 这里会一直展开下去。**当前版本还没有预算/深度限制**；将来接入预算的
/// 位置就是 [`Dispatcher::apply`]。
pub struct Dispatcher {
    registry: Registry,
}

impl Dispatcher {
    /// 用一个注册表构造调度器。
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// 只读访问注册表。
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// 展开一个节点，并递归处理它展开出来的子节点。
    pub fn expand(&self, node: Node, ctx: &mut Context<'_>) -> Node {
        match node {
            Node::Call(call) => {
                let handler = self
                    .registry
                    .get(&call.name)
                    .unwrap_or_else(|| self.registry.fallback());
                self.apply(handler, Block::Call(call), ctx)
            }
            Node::Natural(natural) => {
                let handler = self
                    .registry
                    .natural()
                    .unwrap_or_else(|| self.registry.fallback());
                self.apply(handler, Block::Natural(natural), ctx)
            }
            Node::Element(mut element) => {
                element.children = self.expand_all(element.children, ctx);
                Node::Element(element)
            }
            Node::Fragment(children) => Node::Fragment(self.expand_all(children, ctx)),
            // 已展开的叶子。
            leaf @ (Node::Text(_) | Node::Error(_)) => leaf,
        }
    }

    /// 交给展开器推进一级，再继续展开替换结果。
    fn apply(&self, handler: &dyn Handler, block: Block, ctx: &mut Context<'_>) -> Node {
        let replacement = handler.expand(block, ctx);
        self.expand(replacement, ctx)
    }

    /// 逐个展开一串节点。
    ///
    /// 展开器返回的 [`Node::Fragment`] 会被**摊平**到这一层序列里：一个块
    /// 展开成多个兄弟节点，在最终的树里是字面成立的，渲染器不需要再处理
    /// `Fragment`（也不会在最终树里遇到它）。
    pub fn expand_all(&self, nodes: Vec<Node>, ctx: &mut Context<'_>) -> Vec<Node> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match self.expand(node, ctx) {
                Node::Fragment(children) => out.extend(children),
                other => out.push(other),
            }
        }
        out
    }

    /// 从语法树出发：先播种成节点，再展开。
    pub fn run(&self, blocks: Vec<Block>, ctx: &mut Context<'_>) -> Vec<Node> {
        self.expand_all(Node::seed_all(blocks), ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Attr, CallBlock, Element, ErrorKind, ErrorNode, NaturalBlock, Span};
    use crate::dispatch::Handler;
    use crate::parse::parse_blocks;

    /// 只关心调用块：把子块移交出去，于是子节点会被继续展开。
    struct NoticeHandler;

    impl Handler for NoticeHandler {
        fn expand_call(&self, call: CallBlock, _ctx: &mut Context<'_>) -> Node {
            let mut attrs = vec![Attr::new("class", "notice")];
            if let Some(kind) = call.params.get("type") {
                attrs.push(Attr::new("class", format!("notice-{kind}")));
            }
            Node::Element(Element {
                tag: "div".into(),
                attrs,
                children: Node::seed_all(call.body),
            })
        }
    }

    /// 只关心自然块。
    struct ParagraphHandler;

    impl Handler for ParagraphHandler {
        fn expand_natural(&self, natural: NaturalBlock, _ctx: &mut Context<'_>) -> Node {
            Node::Element(Element {
                tag: "p".into(),
                attrs: Vec::new(),
                children: vec![Node::Text(natural.text)],
            })
        }
    }

    /// 只取原文，直接丢掉 `call.body`：内层永远不会被展开。
    struct CodeHandler;

    impl Handler for CodeHandler {
        fn expand_call(&self, call: CallBlock, _ctx: &mut Context<'_>) -> Node {
            Node::Element(Element {
                tag: "pre".into(),
                attrs: Vec::new(),
                children: vec![Node::Element(Element {
                    tag: "code".into(),
                    attrs: Vec::new(),
                    children: vec![Node::Text(call.raw_body)],
                })],
            })
        }
    }

    /// 展开成两个兄弟节点。
    struct SplitHandler;

    impl Handler for SplitHandler {
        fn expand_call(&self, call: CallBlock, _ctx: &mut Context<'_>) -> Node {
            Node::Fragment(vec![
                Node::Text(format!("[{}]", call.name)),
                Node::Text(call.raw_body),
            ])
        }
    }

    /// 展开器自己判定无法展开。
    struct PickyHandler;

    impl Handler for PickyHandler {
        fn expand_call(&self, call: CallBlock, _ctx: &mut Context<'_>) -> Node {
            Node::Error(ErrorNode::new(
                ErrorKind::ExpandFailed,
                "参数不全",
                call.span,
                call.raw_body,
            ))
        }
    }

    fn contains_error(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Error(_) => true,
            Node::Element(element) => contains_error(&element.children),
            Node::Fragment(children) => contains_error(children),
            _ => false,
        })
    }

    #[test]
    fn a_registered_natural_handler_is_used() {
        let source = "正文\n";
        let mut registry = Registry::new();
        registry.register_natural(ParagraphHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        let Node::Element(p) = &nodes[0] else {
            panic!("期望段落元素");
        };
        assert_eq!(p.tag, "p");
        assert!(matches!(&p.children[0], Node::Text(t) if t == "正文"));
    }

    #[test]
    fn an_unhandled_natural_block_falls_back_with_its_content() {
        // 自然块没有超然待遇：没注册自然块展开器时，它和未知调用块一样走兜底。
        let source = "正文\n";
        let dispatcher = Dispatcher::new(Registry::new());
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        let Node::Error(error) = &nodes[0] else {
            panic!("期望报错节点");
        };
        assert_eq!(error.kind, ErrorKind::NoHandler);
        assert_eq!(error.span.slice(source), "正文");
        assert_eq!(error.content, "正文");
    }

    #[test]
    fn an_unhandled_call_block_falls_back_with_its_content() {
        let source = "::nope a=1:\n  body\n";
        let dispatcher = Dispatcher::new(Registry::new());
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        let Node::Error(error) = &nodes[0] else {
            panic!("期望报错节点");
        };
        assert_eq!(error.kind, ErrorKind::NoHandler);
        assert!(error.message.contains("::nope"));
        // 兜底展开器把整块原文塞进了报错节点，块体内容没有丢。
        assert_eq!(error.content, "::nope a=1:\n  body");
        assert_eq!(error.span, Span::new(1, 2, 0, source.trim_end().len()));
    }

    #[test]
    fn natural_and_call_blocks_are_dispatched_independently() {
        let source = "正文\n\n::notice type=warning:\n  小心\n";
        let mut registry = Registry::new();
        registry.register("notice", NoticeHandler);
        registry.register_natural(ParagraphHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], Node::Element(e) if e.tag == "p"));
        assert!(matches!(&nodes[1], Node::Element(e) if e.tag == "div"));
        assert!(!contains_error(&nodes));
    }

    #[test]
    fn the_fallback_can_be_replaced() {
        let source = "::nope:\n";
        let mut registry = Registry::new();
        registry.set_fallback(NoticeHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        assert!(matches!(&nodes[0], Node::Element(e) if e.tag == "div"));
    }

    #[test]
    fn handing_over_the_body_keeps_expanding_nested_calls() {
        let source = "::notice type=warning:\n  ::notice:\n    内层\n";
        let mut registry = Registry::new();
        registry.register("notice", NoticeHandler);
        registry.register_natural(ParagraphHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        let Node::Element(outer) = &nodes[0] else {
            panic!("期望外层元素");
        };
        assert_eq!(outer.tag, "div");
        assert_eq!(
            outer.attrs,
            vec![
                Attr::new("class", "notice"),
                Attr::new("class", "notice-warning")
            ]
        );

        // 内层调用块被继续展开，它里面的自然块也被继续展开成段落。
        let Node::Element(inner) = &outer.children[0] else {
            panic!("期望内层元素");
        };
        let Node::Element(paragraph) = &inner.children[0] else {
            panic!("期望内层段落");
        };
        assert_eq!(paragraph.tag, "p");
        assert!(matches!(&paragraph.children[0], Node::Text(t) if t == "内层"));
        assert!(!contains_error(&nodes));
    }

    #[test]
    fn a_verbatim_handler_swallows_its_children() {
        // 这一条是整个架构的关键：::code 里的 ::notice 是**字面文本**，
        // 既不该被展开，也不该被当成未知块报错。
        let source = "::code lang=neomark:\n  ::notice type=warning:\n    小心\n";
        let mut registry = Registry::new();
        registry.register("notice", NoticeHandler);
        registry.register("code", CodeHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        let Node::Element(pre) = &nodes[0] else {
            panic!("期望 pre 元素");
        };
        let Node::Element(code) = &pre.children[0] else {
            panic!("期望 code 元素");
        };
        let Node::Text(text) = &code.children[0] else {
            panic!("期望字面文本");
        };
        assert!(text.contains("::notice type=warning:"));
        assert!(!contains_error(&nodes));
    }

    #[test]
    fn a_handler_may_expand_into_several_siblings() {
        let source = "::twice:\n  hi\n";
        let mut registry = Registry::new();
        registry.register("twice", SplitHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        assert_eq!(
            nodes,
            vec![Node::Text("[twice]".into()), Node::Text("hi".into())]
        );
    }

    #[test]
    fn a_handler_may_fail_on_its_own() {
        let source = "::picky:\n";
        let mut registry = Registry::new();
        registry.register("picky", PickyHandler);
        let dispatcher = Dispatcher::new(registry);
        let mut ctx = Context::new(source);

        let nodes = dispatcher.run(parse_blocks(source), &mut ctx);

        assert!(matches!(&nodes[0], Node::Error(e) if e.kind == ErrorKind::ExpandFailed));
    }
}
