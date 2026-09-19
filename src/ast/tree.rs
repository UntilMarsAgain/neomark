//! 一棵保存在 `indextree::Arena` 里的块树。
//!
//! 与手写 `Vec<节点>` 的关系：**孩子不再存在节点自己身上**，而是由 arena 的
//! 父子边表示。所以 [`NodeKind`] 里没有任何 `children` 字段，`Call` 的块体、
//! `Element` 的孩子都是 arena 边。
//!
//! # 为什么有一个文档根
//!
//! `Arena::roots()` 是按**槽位顺序**（创建顺序）遍历的，而不是文档顺序；
//! 一旦展开器在中间新建节点，森林根的顺序就会错。所以这里用一个
//! [`NodeKind::Document`] 根把所有块串成一棵真正的树，顺序由 arena 的兄弟
//! 链表保证，同时调度器替换节点时也永远有父节点可用（不必特判根位置）。

use indextree::{Arena, NodeId};

use super::block::{Block, CallBlock, NaturalBlock};
use super::error::ErrorNode;

/// 节点载荷。
///
/// 除文档根外只有一根轴：**展开了没有**。[`NodeKind::Unparsed`] 是展开的
/// 输入（自然块或调用块，语法分类在 [`Block`] 里），其余都是展开的产物。
/// 语法层将来增加新的块种类时，这里与渲染器都不用改。
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// 文档根：整棵树的唯一根。不参与展开，遍历从它的子节点开始。
    Document,
    /// **未解析**的块；具体是哪一种语法块由 [`Block`] 说明。
    Unparsed(Block),
    /// 已展开的 HTML 元素；元素的孩子就是它的子节点。
    Element {
        /// 标签名，例如 `div`。
        tag: String,
        /// 属性。
        attrs: Vec<Attr>,
    },
    /// 已展开的纯文本（**未转义**；转义是渲染器的事）。
    Text(String),
    /// 报错节点：展开器无法工作时的**最终回退**，是叶子。
    Error(ErrorNode),
}

/// 「这个节点该走哪条路」的标签，**不是**载荷形状的镜像。
///
/// 它是 `Copy` 的，用于在不持有 `&Ast` 的情况下决定去向——拿 `&NodeKind`
/// 的同时又需要 `&mut Ast` 会被借用检查拒绝，而标签不会。
///
/// 因此这里刻意**摊平**：`NodeKind::Unparsed(Block)` 一个变体，对应
/// [`KindTag::Natural`] / [`KindTag::Call`] 两个标签，因为调度器必须知道
/// 该按名字查表还是走自然块槽位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindTag {
    /// 文档根。
    Document,
    /// 未解析的自然块：没有名字，走专门槽位。
    Natural,
    /// 未解析的调用块：按名字查找。
    Call,
    /// 已展开的元素。
    Element,
    /// 已展开的文本。
    Text,
    /// 报错节点。
    Error,
}

impl NodeKind {
    /// 种类标签。
    pub const fn tag(&self) -> KindTag {
        match self {
            NodeKind::Document => KindTag::Document,
            NodeKind::Unparsed(Block::Natural(_)) => KindTag::Natural,
            NodeKind::Unparsed(Block::Call(_)) => KindTag::Call,
            NodeKind::Element { .. } => KindTag::Element,
            NodeKind::Text(_) => KindTag::Text,
            NodeKind::Error(_) => KindTag::Error,
        }
    }

    /// 是否是尚未解析的块。
    pub const fn is_unparsed(&self) -> bool {
        matches!(self, NodeKind::Unparsed(_))
    }
}

/// 一棵块树。
#[derive(Debug, Clone)]
pub struct Ast {
    arena: Arena<NodeKind>,
    document: NodeId,
}

impl Default for Ast {
    fn default() -> Self {
        Self::new()
    }
}

impl Ast {
    /// 新建一棵只有文档根的树。
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let document = arena.new_node(NodeKind::Document);
        Self { arena, document }
    }

    /// 文档根。整棵树的遍历都从它的子节点开始。
    pub fn document(&self) -> NodeId {
        self.document
    }

    /// 底层的 arena。展开器需要兄弟导航、重新挂载、删除子树等操作时用它。
    pub fn arena(&self) -> &Arena<NodeKind> {
        &self.arena
    }

    /// 底层的 arena（可变）。
    pub fn arena_mut(&mut self) -> &mut Arena<NodeKind> {
        &mut self.arena
    }

    /// 树结构自检（用于测试）。
    pub fn validate(&self) -> bool {
        self.arena.validate()
    }

    // ── 读取 ───────────────────────────────────────────────

    /// 节点的载荷。
    pub fn kind(&self, id: NodeId) -> Option<&NodeKind> {
        self.arena.get_data(id)
    }

    /// 节点的载荷（可变）。
    pub fn kind_mut(&mut self, id: NodeId) -> Option<&mut NodeKind> {
        self.arena.get_data_mut(id)
    }

    /// 节点的种类标签。
    pub fn tag(&self, id: NodeId) -> Option<KindTag> {
        self.arena.get_data(id).map(NodeKind::tag)
    }

    /// 节点是否是尚未解析的块。
    pub fn is_unparsed(&self, id: NodeId) -> bool {
        self.arena.get_data(id).is_some_and(NodeKind::is_unparsed)
    }

    /// 未解析块的语法载荷。
    pub fn block(&self, id: NodeId) -> Option<&Block> {
        match self.arena.get_data(id)? {
            NodeKind::Unparsed(block) => Some(block),
            _ => None,
        }
    }

    /// 自然块的载荷。
    pub fn natural(&self, id: NodeId) -> Option<&NaturalBlock> {
        match self.block(id)? {
            Block::Natural(natural) => Some(natural),
            Block::Call(_) => None,
        }
    }

    /// 调用块的载荷。
    pub fn call(&self, id: NodeId) -> Option<&CallBlock> {
        match self.block(id)? {
            Block::Call(call) => Some(call),
            Block::Natural(_) => None,
        }
    }

    /// 元素的标签与属性。
    pub fn element(&self, id: NodeId) -> Option<(&str, &[Attr])> {
        match self.arena.get_data(id)? {
            NodeKind::Element { tag, attrs } => Some((tag, attrs)),
            _ => None,
        }
    }

    /// 纯文本。
    pub fn text(&self, id: NodeId) -> Option<&str> {
        match self.arena.get_data(id)? {
            NodeKind::Text(text) => Some(text),
            _ => None,
        }
    }

    /// 报错节点。
    pub fn error(&self, id: NodeId) -> Option<&ErrorNode> {
        match self.arena.get_data(id)? {
            NodeKind::Error(error) => Some(error),
            _ => None,
        }
    }

    // ── 建立 ───────────────────────────────────────────────

    /// 新建一个任意载荷的节点（**游离**状态，尚未挂进树）。
    pub fn new_node(&mut self, kind: NodeKind) -> NodeId {
        self.arena.new_node(kind)
    }

    /// 新建一个元素节点。
    pub fn new_element(&mut self, tag: impl Into<String>, attrs: Vec<Attr>) -> NodeId {
        self.new_node(NodeKind::Element {
            tag: tag.into(),
            attrs,
        })
    }

    /// 新建一个文本节点。
    pub fn new_text(&mut self, text: impl Into<String>) -> NodeId {
        self.new_node(NodeKind::Text(text.into()))
    }

    /// 新建一个报错节点。
    pub fn new_error(&mut self, error: ErrorNode) -> NodeId {
        self.new_node(NodeKind::Error(error))
    }

    // ── 结构操作 ───────────────────────────────────────────

    /// 把 `child` 挂为 `parent` 的最后一个孩子。
    ///
    /// 如果 `child` 本来就挂在别处，`indextree` 会先把它摘下来——所以
    /// 「把块体移交给新元素」就是一次 `append`。
    pub fn append(&mut self, parent: NodeId, child: NodeId) {
        parent.append(child, &mut self.arena);
    }

    /// 把 `node` 插到 `anchor` 之后（成为其下一个兄弟）。
    pub fn insert_after(&mut self, anchor: NodeId, node: NodeId) {
        anchor.insert_after(node, &mut self.arena);
    }

    /// 删除 `node` 及其整棵子树。
    pub fn remove_subtree(&mut self, node: NodeId) {
        node.remove_subtree(&mut self.arena);
    }

    /// 遍历 `parent` 的孩子（文档顺序）。
    pub fn children(&self, parent: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        parent.children(&self.arena)
    }

    /// 父节点。
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        id.parent(&self.arena)
    }

    /// 把一个块挂到文档根下。
    pub fn push_block(&mut self, id: NodeId) {
        self.append(self.document, id);
    }
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
