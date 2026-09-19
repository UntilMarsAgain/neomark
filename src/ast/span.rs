//! 块在源文本中的位置。

/// 一个块在原始输入中的位置范围。
///
/// * 行号从 1 开始、含首尾；
/// * 字节偏移基于**原始输入**，`end_byte` 为开区间，且不含行尾换行。
///
/// 嵌套在调用块内部的块，其位置同样是原始输入里的绝对位置，
/// 而不是相对去缩进块的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 起始行号（从 1 开始，含）。
    pub start_line: usize,
    /// 结束行号（从 1 开始，含）。
    pub end_line: usize,
    /// 起始字节偏移（含）。
    pub start_byte: usize,
    /// 结束字节偏移（不含）。
    pub end_byte: usize,
}

impl Span {
    /// 直接构造一个位置范围。
    pub const fn new(
        start_line: usize,
        end_line: usize,
        start_byte: usize,
        end_byte: usize,
    ) -> Self {
        Self {
            start_line,
            end_line,
            start_byte,
            end_byte,
        }
    }

    /// 跨越的字节数。
    pub const fn len_bytes(&self) -> usize {
        self.end_byte.saturating_sub(self.start_byte)
    }

    /// 跨越的行数（含首尾）。
    pub const fn len_lines(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// 从原始输入里切出这一段。
    ///
    /// 位置由解析器产生，必定落在行边界上，因此不会切坏 UTF-8；
    /// 但手工构造的非法 `Span` 会让它 panic。
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start_byte..self.end_byte]
    }
}
