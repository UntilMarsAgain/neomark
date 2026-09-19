//! 块切分与递归解析：直接建出一棵 [`Ast`]。
//!
//! # 切分规则
//!
//! * 自然块由空行切分；调用块由缩进切分。
//! * 任何一行（忽略前导缩进）以 `::` 开头，就视为调用块的头部行，
//!   即使它前面没有空行。
//! * 头部行之后，缩进**严格大于**头部行缩进的行属于该调用块的块体；
//!   一旦缩进不再严格大于，该行就是下一个块的首行，无论末尾有没有空行。
//! * 调用块体内部的空行不结束块：只要后面还有更深缩进的内容行，块就继续；
//!   末尾的连续空行不算内容。
//! * 块体在去掉公共缩进后被递归解析，因此调用块可以嵌套——块体就是该调用
//!   节点在 arena 里的子节点。

use crate::ast::{Ast, CallBlock, NaturalBlock, NodeId, NodeKind, Span};
use crate::parse::header::parse_call_header;
use crate::parse::line::{SrcLine, scan_lines};

/// 把一段 neomark 文本解析成一棵块树。
///
/// 顶层块挂在文档根（[`Ast::document`]）下。
///
/// # 示例
///
/// ```
/// use neomark::{KindTag, parse};
///
/// let ast = parse("你好。\n\n::notice type=warning:\n  小心！");
/// let blocks: Vec<_> = ast.children(ast.document()).collect();
///
/// assert_eq!(blocks.len(), 2);
/// assert_eq!(ast.tag(blocks[0]), Some(KindTag::Natural));
/// assert_eq!(ast.call(blocks[1]).unwrap().name, "notice");
/// assert_eq!(ast.call(blocks[1]).unwrap().param("type"), Some("warning"));
/// ```
pub fn parse(input: &str) -> Ast {
    let mut ast = Ast::new();
    let lines = scan_lines(input);

    for id in parse_sequence(&mut ast, &lines) {
        ast.push_block(id);
    }

    ast
}

/// 解析一段行序列，识别并分发自然块 / 调用块，返回按文档顺序排列的节点。
fn parse_sequence(ast: &mut Ast, lines: &[SrcLine<'_>]) -> Vec<NodeId> {
    let mut ids = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        // 空行只是分隔符。
        if lines[index].blank {
            index += 1;
            continue;
        }

        let (id, next) = if lines[index].is_call() {
            parse_call(ast, lines, index)
        } else {
            parse_natural(ast, lines, index)
        };
        ids.push(id);
        index = next;
    }

    ids
}

/// 收集一个自然块：直到空行、调用行首或文本结束。
fn parse_natural(ast: &mut Ast, lines: &[SrcLine<'_>], start: usize) -> (NodeId, usize) {
    let mut end = start;
    while end < lines.len() && !lines[end].blank && !lines[end].is_call() {
        end += 1;
    }

    let id = ast.new_node(NodeKind::Natural(NaturalBlock {
        text: join_lines(&lines[start..end]),
        span: span_of(&lines[start..end]),
    }));
    (id, end)
}

/// 解析一个调用块：头部 + 按缩进界定的块体（递归解析成子节点）。
fn parse_call(ast: &mut Ast, lines: &[SrcLine<'_>], start: usize) -> (NodeId, usize) {
    let header_line = &lines[start];
    let header_indent = header_line.indent;
    let header = parse_call_header(header_line.content());

    // 向后找块体：空行跳过但不算结束，缩进严格更大的行算内容。
    // `body_end` 始终停在“最后一行内容行 + 1”，从而自动丢掉末尾空行。
    let mut cursor = start + 1;
    let mut body_end = start + 1;
    while cursor < lines.len() {
        let line = &lines[cursor];
        if line.blank {
            cursor += 1;
            continue;
        }
        if line.indent > header_indent {
            body_end = cursor + 1;
            cursor += 1;
            continue;
        }
        break;
    }

    // 块体开头的空行同样不算内容。
    let mut body_start = start + 1;
    while body_start < body_end && lines[body_start].blank {
        body_start += 1;
    }

    // 去掉公共缩进后递归解析；块体就是调用节点的子节点。
    let dedented = dedent(&lines[body_start..body_end]);
    let children = parse_sequence(ast, &dedented);
    let raw_body = join_lines(&dedented);

    // 没有块体时 `body_end == start + 1`，这里正好落回头部行自身。
    let last = &lines[body_end - 1];
    let span = Span::new(
        header_line.line_no,
        last.line_no,
        header_line.start,
        last.end,
    );

    let id = ast.new_node(NodeKind::Call(CallBlock {
        name: header.name,
        params: header.params,
        raw_body,
        span,
    }));
    for child in children {
        ast.append(id, child);
    }

    (id, cursor)
}

/// 一组行在原文中的位置范围。
fn span_of(lines: &[SrcLine<'_>]) -> Span {
    let first = &lines[0];
    let last = &lines[lines.len() - 1];
    Span::new(first.line_no, last.line_no, first.start, last.end)
}

/// 按非空行的最小缩进整体去缩进；空行被规范化为空文本。
///
/// 行号与字节偏移原样保留，所以嵌套块的位置仍是原文里的绝对位置。
fn dedent<'a>(lines: &[SrcLine<'a>]) -> Vec<SrcLine<'a>> {
    let min_indent = lines
        .iter()
        .filter(|line| !line.blank)
        .map(|line| line.indent)
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|line| {
            if line.blank {
                SrcLine {
                    text: "",
                    indent: 0,
                    blank: true,
                    line_no: line.line_no,
                    start: line.start,
                    end: line.end,
                }
            } else {
                let strip = min_indent.min(line.indent);
                SrcLine {
                    text: &line.text[strip..],
                    indent: line.indent - strip,
                    blank: false,
                    line_no: line.line_no,
                    start: line.start,
                    end: line.end,
                }
            }
        })
        .collect()
}

/// 把若干行用 `\n` 连接成一段文本。
fn join_lines(lines: &[SrcLine<'_>]) -> String {
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_util::sexpr;

    #[test]
    fn empty_input_has_no_blocks() {
        assert_eq!(sexpr(&parse("")), "");
        assert_eq!(sexpr(&parse("\n\n   \n")), "");
    }

    #[test]
    fn natural_blocks_split_on_blank_lines() {
        let ast = parse("第一段\n仍然第一段\n\n\n第二段");
        assert_eq!(
            sexpr(&ast),
            r#"natural("第一段\n仍然第一段") natural("第二段")"#
        );
    }

    #[test]
    fn crlf_is_supported() {
        let ast = parse("a\r\n\r\n::x:\r\n  b\r\n");
        assert_eq!(sexpr(&ast), r#"natural("a") (call x natural("b"))"#);
    }

    #[test]
    fn call_line_starts_a_block_without_a_preceding_blank_line() {
        // 规则 1：以 :: 开头的行永远开启调用块，哪怕前面没有空行。
        let ast = parse("正文\n::notice:\n  内容");
        assert_eq!(
            sexpr(&ast),
            r#"natural("正文") (call notice natural("内容"))"#
        );
    }

    #[test]
    fn dedent_ends_a_call_block_without_a_blank_line() {
        // 规则 3：缩进不严格大于即为下一个块的首行，末尾无需空行。
        let ast = parse("::a:\n  x\n下一块");
        assert_eq!(sexpr(&ast), r#"(call a natural("x")) natural("下一块")"#);
    }

    #[test]
    fn blank_lines_inside_a_call_body_do_not_end_it() {
        let ast = parse("::a:\n  x\n\n  y\n\n结尾");
        assert_eq!(
            sexpr(&ast),
            r#"(call a natural("x") natural("y")) natural("结尾")"#
        );
    }

    #[test]
    fn trailing_blank_lines_are_not_part_of_the_body() {
        let ast = parse("::a:\n  x\n\n\n");
        assert_eq!(sexpr(&ast), r#"(call a natural("x"))"#);
    }

    #[test]
    fn call_block_without_a_body() {
        let ast = parse("::a:\n::b:\n");
        assert_eq!(sexpr(&ast), "call a call b");
    }

    #[test]
    fn call_blocks_nest_by_indentation() {
        let ast = parse("::outer:\n  ::inner:\n    deep\n  tail");
        assert_eq!(
            sexpr(&ast),
            r#"(call outer (call inner natural("deep")) natural("tail"))"#
        );
    }

    #[test]
    fn indented_call_line_interrupts_a_natural_block() {
        let ast = parse("文字\n  ::x:\n    y\n后续");
        assert_eq!(
            sexpr(&ast),
            r#"natural("文字") (call x natural("y")) natural("后续")"#
        );
    }

    #[test]
    fn body_keeps_relative_indentation() {
        let ast = parse("::a:\n    deep\n  shallow");
        assert_eq!(sexpr(&ast), r#"(call a natural("  deep\nshallow"))"#);
    }

    #[test]
    fn spans_and_raw_body_point_at_the_original_source() {
        let source = "::code lang=rust:\n  fn main() {}\n";
        let ast = parse(source);
        let root = ast.children(ast.document()).next().unwrap();
        let call = ast.call(root).unwrap();

        assert_eq!(call.span, Span::new(1, 2, 0, source.trim_end().len()));
        assert_eq!(call.raw_body, "fn main() {}");
        assert_eq!(call.span.slice(source), "::code lang=rust:\n  fn main() {}");
    }

    #[test]
    fn nested_block_spans_are_absolute() {
        let source = "::outer:\n  ::inner:\n    x\n";
        let ast = parse(source);
        let outer = ast.children(ast.document()).next().unwrap();
        let inner = ast.children(outer).next().unwrap();

        assert_eq!(ast.call(outer).unwrap().span, Span::new(1, 3, 0, 25));
        assert_eq!(
            ast.call(outer).unwrap().span.slice(source),
            "::outer:\n  ::inner:\n    x"
        );
        assert_eq!(ast.call(inner).unwrap().span, Span::new(2, 3, 9, 25));
        assert_eq!(
            ast.call(inner).unwrap().span.slice(source),
            "  ::inner:\n    x"
        );
        assert_eq!(ast.call(inner).unwrap().raw_body, "x");
    }

    #[test]
    fn the_tree_passes_the_arena_integrity_check() {
        let ast = parse("正文\n\n::outer:\n  ::inner:\n    deep\n  tail\n");
        assert!(ast.validate());
    }

    #[test]
    fn the_specification_example() {
        let source = "\
::code lang=neomark:
  ::notice type=warning:
    这是一个通知调用块，参数 type 的值为 \"warning\"

  ::notice title=\"Be Careful!\":
    需要缩进表示属于调用体

  ::image width=300 height=200 lazy:
    path/to/image.jpg
  图片调用块，包含三个参数：width、height 和 lazy（等同于 lazy=true）

  ::quote author=\"张三\" source=\"《文章标题》\":
    这里是引用内容
    可以有多行";

        let expected = r#"(call code lang=neomark (call notice type=warning natural("这是一个通知调用块，参数 type 的值为 \"warning\"")) (call notice title=Be Careful! natural("需要缩进表示属于调用体")) (call image width=300 height=200 lazy=true natural("path/to/image.jpg")) natural("图片调用块，包含三个参数：width、height 和 lazy（等同于 lazy=true）") (call quote author=张三 source=《文章标题》 natural("这里是引用内容\n可以有多行")))"#;

        assert_eq!(sexpr(&parse(source)), expected);
    }
}
