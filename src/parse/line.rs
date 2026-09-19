//! 行扫描：把输入切成行，并算出每行的缩进、空行标记与原文位置。
//!
//! 这一层是纯词法的，不关心块如何切分；切分逻辑建在它之上。

/// 扫描出的一个行。
///
/// `text` 可能已经被去缩进处理过，但 `indent` 始终与 `text` 的前导空格数一致；
/// `line_no` / `start` / `end` 则始终指向**原始输入**，不受去缩进影响。
#[derive(Debug, Clone, Copy)]
pub(crate) struct SrcLine<'a> {
    pub(crate) text: &'a str,
    pub(crate) indent: usize,
    pub(crate) blank: bool,
    /// 原文行号（从 1 开始）。
    pub(crate) line_no: usize,
    /// 原文行首字节偏移。
    pub(crate) start: usize,
    /// 原文行尾字节偏移（不含行尾换行）。
    pub(crate) end: usize,
}

impl<'a> SrcLine<'a> {
    fn new(text: &'a str, line_no: usize, start: usize, end: usize) -> Self {
        Self {
            text,
            indent: count_indent(text),
            blank: text.trim().is_empty(),
            line_no,
            start,
            end,
        }
    }

    /// 去掉行首缩进后的内容。
    pub(crate) fn content(&self) -> &'a str {
        &self.text[self.indent..]
    }

    /// 这一行是否是调用块头部行。
    pub(crate) fn is_call(&self) -> bool {
        !self.blank && self.content().starts_with("::")
    }
}

/// 只统计行首的 ASCII 空格；制表符不算缩进。
fn count_indent(text: &str) -> usize {
    text.bytes().take_while(|&b| b == b' ').count()
}

/// 按 `\n` 切行，兼容 `\r\n`，末尾换行不产生额外的空行。
pub(crate) fn scan_lines(input: &str) -> Vec<SrcLine<'_>> {
    let bytes = input.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut line_no = 1;

    loop {
        match input[start..].find('\n') {
            Some(relative) => {
                let newline = start + relative;
                let mut end = newline;
                if end > start && bytes[end - 1] == b'\r' {
                    end -= 1;
                }
                lines.push(SrcLine::new(&input[start..end], line_no, start, end));
                start = newline + 1;
                line_no += 1;
                if start >= input.len() {
                    break;
                }
            }
            None => {
                if start < input.len() {
                    let mut end = input.len();
                    if end > start && bytes[end - 1] == b'\r' {
                        end -= 1;
                    }
                    lines.push(SrcLine::new(&input[start..end], line_no, start, end));
                }
                break;
            }
        }
    }

    lines
}
