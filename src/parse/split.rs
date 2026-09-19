//! 块切分与递归解析。
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
//! * 块体在去掉公共缩进后被递归解析，因此调用块可以嵌套。

use crate::ast::{Block, CallBlock, NaturalBlock};
use crate::parse::header::parse_call_header;
use crate::parse::line::{SrcLine, scan_lines};

/// 把一段 neomark 文本切分并解析成块数组。
///
/// 这是本模块的主入口；调用块的块体会被递归解析成子块数组。
///
/// # 示例
///
/// ```
/// use neomark::{parse_blocks, Block};
///
/// let blocks = parse_blocks("你好。\n\n::notice type=warning:\n  小心！");
///
/// assert_eq!(blocks.len(), 2);
/// assert!(matches!(&blocks[0], Block::Natural(b) if b.text == "你好。"));
/// assert!(matches!(&blocks[1], Block::Call(b) if b.name == "notice"
///     && b.params.get("type") == Some("warning")));
/// ```
pub fn parse_blocks(input: &str) -> Vec<Block> {
    let lines = scan_lines(input);
    parse_sequence(&lines)
}

/// 解析一段行序列，识别并分发自然块 / 调用块。
fn parse_sequence(lines: &[SrcLine<'_>]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        // 空行只是分隔符。
        if lines[index].blank {
            index += 1;
            continue;
        }

        let (block, next) = if lines[index].is_call() {
            parse_call(lines, index)
        } else {
            parse_natural(lines, index)
        };
        blocks.push(block);
        index = next;
    }

    blocks
}

/// 收集一个自然块：直到空行、调用行首或文本结束。
fn parse_natural(lines: &[SrcLine<'_>], start: usize) -> (Block, usize) {
    let mut end = start;
    while end < lines.len() && !lines[end].blank && !lines[end].is_call() {
        end += 1;
    }

    let block = NaturalBlock {
        text: join_lines(&lines[start..end]),
    };
    (Block::Natural(block), end)
}

/// 解析一个调用块：头部 + 按缩进界定的块体（递归解析）。
fn parse_call(lines: &[SrcLine<'_>], start: usize) -> (Block, usize) {
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

    // 去掉公共缩进后递归解析，实现“每次展开一层”的嵌套。
    let dedented = dedent(&lines[body_start..body_end]);
    let body = parse_sequence(&dedented);

    let block = CallBlock {
        name: header.name,
        params: header.params,
        body,
    };
    (Block::Call(block), cursor)
}

/// 按非空行的最小缩进整体去缩进；空行被规范化为空文本。
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
                }
            } else {
                let strip = min_indent.min(line.indent);
                SrcLine {
                    text: &line.text[strip..],
                    indent: line.indent - strip,
                    blank: false,
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

    fn natural(text: &str) -> Block {
        Block::Natural(NaturalBlock {
            text: text.to_string(),
        })
    }

    fn call(name: &str, params: &[(&str, &str)], body: Vec<Block>) -> Block {
        Block::Call(CallBlock {
            name: name.to_string(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body,
        })
    }

    #[test]
    fn empty_input_has_no_blocks() {
        assert!(parse_blocks("").is_empty());
        assert!(parse_blocks("\n\n   \n").is_empty());
    }

    #[test]
    fn natural_blocks_split_on_blank_lines() {
        let blocks = parse_blocks("第一段\n仍然第一段\n\n\n第二段");
        assert_eq!(
            blocks,
            vec![natural("第一段\n仍然第一段"), natural("第二段")]
        );
    }

    #[test]
    fn crlf_is_supported() {
        let blocks = parse_blocks("a\r\n\r\n::x:\r\n  b\r\n");
        assert_eq!(
            blocks,
            vec![natural("a"), call("x", &[], vec![natural("b")])]
        );
    }

    #[test]
    fn call_line_starts_a_block_without_a_preceding_blank_line() {
        // 规则 1：以 :: 开头的行永远开启调用块，哪怕前面没有空行。
        let blocks = parse_blocks("正文\n::notice:\n  内容");
        assert_eq!(
            blocks,
            vec![natural("正文"), call("notice", &[], vec![natural("内容")])]
        );
    }

    #[test]
    fn dedent_ends_a_call_block_without_a_blank_line() {
        // 规则 3：缩进不严格大于即为下一个块的首行，末尾无需空行。
        let blocks = parse_blocks("::a:\n  x\n下一块");
        assert_eq!(
            blocks,
            vec![call("a", &[], vec![natural("x")]), natural("下一块")]
        );
    }

    #[test]
    fn blank_lines_inside_a_call_body_do_not_end_it() {
        let blocks = parse_blocks("::a:\n  x\n\n  y\n\n结尾");
        assert_eq!(
            blocks,
            vec![
                call("a", &[], vec![natural("x"), natural("y")]),
                natural("结尾"),
            ]
        );
    }

    #[test]
    fn trailing_blank_lines_are_not_part_of_the_body() {
        let blocks = parse_blocks("::a:\n  x\n\n\n");
        assert_eq!(blocks, vec![call("a", &[], vec![natural("x")])]);
    }

    #[test]
    fn call_block_without_a_body() {
        let blocks = parse_blocks("::a:\n::b:\n");
        assert_eq!(blocks, vec![call("a", &[], vec![]), call("b", &[], vec![])]);
    }

    #[test]
    fn call_blocks_nest_by_indentation() {
        let blocks = parse_blocks("::outer:\n  ::inner:\n    deep\n  tail");
        assert_eq!(
            blocks,
            vec![call(
                "outer",
                &[],
                vec![call("inner", &[], vec![natural("deep")]), natural("tail"),]
            )]
        );
    }

    #[test]
    fn indented_call_line_interrupts_a_natural_block() {
        let blocks = parse_blocks("文字\n  ::x:\n    y\n后续");
        assert_eq!(
            blocks,
            vec![
                natural("文字"),
                call("x", &[], vec![natural("y")]),
                natural("后续"),
            ]
        );
    }

    #[test]
    fn body_keeps_relative_indentation() {
        let blocks = parse_blocks("::a:\n    deep\n  shallow");
        assert_eq!(
            blocks,
            vec![call("a", &[], vec![natural("  deep\nshallow")])]
        );
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

        let blocks = parse_blocks(source);
        assert_eq!(
            blocks,
            vec![call(
                "code",
                &[("lang", "neomark")],
                vec![
                    call(
                        "notice",
                        &[("type", "warning")],
                        vec![natural("这是一个通知调用块，参数 type 的值为 \"warning\"")],
                    ),
                    call(
                        "notice",
                        &[("title", "Be Careful!")],
                        vec![natural("需要缩进表示属于调用体")],
                    ),
                    call(
                        "image",
                        &[("width", "300"), ("height", "200"), ("lazy", "true")],
                        vec![natural("path/to/image.jpg")],
                    ),
                    natural("图片调用块，包含三个参数：width、height 和 lazy（等同于 lazy=true）"),
                    call(
                        "quote",
                        &[("author", "张三"), ("source", "《文章标题》")],
                        vec![natural("这里是引用内容\n可以有多行")],
                    ),
                ]
            )]
        );
    }
}
