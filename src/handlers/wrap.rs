//! 包装器展开器：把调用换成一层元素。
//!
//! 大多数展开器其实只干一件事——「外面套一层别的标签，或者一个带 class 的
//! `div`」。所以这里给注册这一步配个语法糖。

use crate::ast::{Ast, Attr, NodeId};
use crate::dispatch::{Context, Handler};

/// 通用包装器：把调用整个换成一层元素，块体原样搬进去（所以内层照常展开）。
///
/// ```
/// use neomark::{Registry, handlers::Wrap};
/// use regex::Regex;
///
/// let mut registry = Registry::new();
///
/// // 精确名：`::notice` → <div class="nm-notice">…</div>
/// registry.register("notice", Wrap::tag("div").class("nm-notice"));
///
/// // 族名：`::note-info` / `::note-warning` → <div class="note note-info">
/// registry.register_pattern(
///     Regex::new("^note-").unwrap(),
///     Wrap::tag("div").class("note").class_from_name(),
/// );
/// ```
///
/// # 它和内置渲染器的关系
///
/// 它产出的是 [`NodeKind::Element`](crate::NodeKind::Element)，也就是那个
/// **逃生口**：直接写 HTML 标签，不走渲染器的语义映射，也不受 `nm-` 命名约定
/// 约束。想让自家那一族类名与内置输出风格一致，自己给类名加前缀即可。
///
/// 只处理**调用块**。自然块与行内调用走 [`Handler`] 的默认实现（报错节点），
/// 因为「包装一个自然块」的意思不明确——是包住每一段，还是包住整块文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wrap {
    tag: String,
    classes: Vec<String>,
    attrs: Vec<Attr>,
    name_as_class: bool,
}

impl Wrap {
    /// 用给定标签构造。
    pub fn tag(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            classes: Vec::new(),
            attrs: Vec::new(),
            name_as_class: false,
        }
    }

    /// 追加一个类名。
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// 追加一个普通属性。
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push(Attr::new(name, value));
        self
    }

    /// 追加一个布尔属性。
    pub fn flag(mut self, name: impl Into<String>) -> Self {
        self.attrs.push(Attr::boolean(name));
        self
    }

    /// 把**调用名**也作为一个类名加上：`::note-warning` →
    /// `class="note note-warning"`。
    ///
    /// 用调用名而不是**正则实际匹配到的那一段**：模式常常只锚定开头
    /// （`^note-`），匹配到的只是前缀，而该当类名用的是完整的调用名。
    /// 非法字符会被换成 `-`，因为类名不能带空格。
    pub fn class_from_name(mut self) -> Self {
        self.name_as_class = true;
        self
    }

    /// 组装最终的属性表。
    fn attrs_for(&self, name: &str) -> Vec<Attr> {
        let mut classes: Vec<String> = self.classes.clone();
        if self.name_as_class {
            let derived = sanitize_class(name);
            if !derived.is_empty() {
                classes.push(derived);
            }
        }

        let mut attrs = self.attrs.clone();
        if !classes.is_empty() {
            // 类名放最前面，读起来顺眼。
            attrs.insert(0, Attr::new("class", classes.join(" ")));
        }

        attrs
    }
}

impl Handler for Wrap {
    fn expand_call(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(call) = ast.call(node) else {
            return Vec::new();
        };
        let name = call.name.clone();
        let element = ast.new_element(self.tag.clone(), self.attrs_for(&name));

        // 移交块体：一次 append 就把子节点搬过来了，所以内层会照常继续展开。
        for child in ast.children(node).collect::<Vec<_>>() {
            ast.append(element, child);
        }

        vec![element]
    }
}

/// 把调用名变成能当类名用的字符串：只留 `[A-Za-z0-9_-]`，其余换成 `-`。
fn sanitize_class(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_collected_into_one_attribute_in_order() {
        let wrap = Wrap::tag("div").class("a").class("b");
        let attrs = wrap.attrs_for("whatever");

        assert_eq!(attrs, vec![Attr::new("class", "a b")]);
    }

    #[test]
    fn name_as_class_is_appended_last() {
        let wrap = Wrap::tag("div").class("note").class_from_name();

        assert_eq!(
            wrap.attrs_for("note-warning"),
            vec![Attr::new("class", "note note-warning")]
        );
    }

    #[test]
    fn name_as_class_is_sanitized_because_class_names_cannot_contain_spaces() {
        let wrap = Wrap::tag("div").class_from_name();

        // ASCII 部分留着，其余每个字符换成一个 `-`
        assert_eq!(
            wrap.attrs_for("note 警告"),
            vec![Attr::new("class", "note---")]
        );
        // 名字为空时干脆不加这个属性
        assert!(wrap.attrs_for("").is_empty());
    }

    #[test]
    fn extra_attributes_survive_and_class_comes_first() {
        let wrap = Wrap::tag("img")
            .attr("src", "a.png")
            .flag("lazy")
            .class("pic");

        assert_eq!(
            wrap.attrs_for("x"),
            vec![
                Attr::new("class", "pic"),
                Attr::new("src", "a.png"),
                Attr::boolean("lazy"),
            ]
        );
    }
}
