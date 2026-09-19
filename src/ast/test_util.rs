//! 测试用的树形打印：把森林渲染成紧凑的 S 表达式，便于断言结构。

use super::{Ast, Block, NodeId, NodeKind};

/// 把文档根下的森林渲染成一行 S 表达式。
///
/// 格式：无孩子的节点直接打印头部，有孩子的打印 `(头部 孩子...)`。
pub(crate) fn sexpr(ast: &Ast) -> String {
    let roots: Vec<NodeId> = ast.children(ast.document()).collect();
    roots
        .iter()
        .map(|&id| node(ast, id))
        .collect::<Vec<_>>()
        .join(" ")
}

fn node(ast: &Ast, id: NodeId) -> String {
    let head = head(ast, id);
    let children: Vec<String> = ast.children(id).map(|child| node(ast, child)).collect();
    if children.is_empty() {
        head
    } else {
        format!("({head} {})", children.join(" "))
    }
}

fn head(ast: &Ast, id: NodeId) -> String {
    match ast.kind(id) {
        Some(NodeKind::Document) => "document".to_string(),
        Some(NodeKind::Unparsed(Block::Natural(natural))) => {
            format!("natural({:?})", natural.text)
        }
        Some(NodeKind::Unparsed(Block::Call(call))) => {
            let mut out = format!("call {}", call.name);
            for (key, value) in call.params.iter() {
                out.push_str(&format!(" {key}={value}"));
            }
            out
        }
        Some(NodeKind::Element { tag, attrs }) => {
            let mut out = format!("element {tag}");
            for attr in attrs {
                match &attr.value {
                    Some(value) => out.push_str(&format!(" {}={value}", attr.name)),
                    None => out.push_str(&format!(" {}!", attr.name)),
                }
            }
            out
        }
        Some(NodeKind::Text(text)) => format!("text({text:?})"),
        Some(NodeKind::Error(error)) => format!("error({:?})", error.message),
        None => "<gone>".to_string(),
    }
}
