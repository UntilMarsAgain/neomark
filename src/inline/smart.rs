//! 智能标点。
//!
//! 只作用于**普通文本**——代码跨度、数学、转义后的字符都不会经过这里。
//!
//! 引号用**开合状态机**判定，而不是看前一个字符：中文里引号前面直接跟汉字
//! （`他说"好"`），纯上下文启发式会把第一个引号判成闭引号。所以规则是
//! 「下一个引号开，再下一个合」，交替进行；`'` 前面若是字母则一律当撇号，
//! 这样 `don't` → `don’t`。
//!
//! 状态跨段保留，所以扫描器要持有一个实例。

/// 这个字符是否在智能标点的管辖范围内。
///
/// 转义（`\"` `\-` …）的意义就是「原样」，所以扫描器对被转义的这类字符必须
/// 绕开智能标点——直接成型为独立文本，而不是丢进会被转换的缓冲区。
pub(crate) const fn affects(c: char) -> bool {
    matches!(c, '"' | '\'' | '-' | '.')
}

/// 智能标点转换器。
pub(crate) struct Converter {
    enabled: bool,
    next_double_opens: bool,
    next_single_opens: bool,
}

impl Converter {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            next_double_opens: true,
            next_single_opens: true,
        }
    }

    /// 转换一段普通文本。
    pub(crate) fn apply(&mut self, text: &str) -> String {
        if !self.enabled {
            return text.to_string();
        }

        let chars: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            let rest = &chars[i..];

            match c {
                '.' if starts_with(rest, "...") => {
                    out.push('…');
                    i += 3;
                }
                '-' if starts_with(rest, "---") => {
                    out.push('—');
                    i += 3;
                }
                '-' if starts_with(rest, "--") => {
                    out.push('–');
                    i += 2;
                }
                '"' => {
                    out.push(if self.next_double_opens { '“' } else { '”' });
                    self.next_double_opens = !self.next_double_opens;
                    i += 1;
                }
                '\'' => {
                    if previous(&chars, i).is_some_and(|p| p.is_alphanumeric()) {
                        // 撇号，不影响引号状态
                        out.push('’');
                    } else if self.next_single_opens {
                        out.push('‘');
                        self.next_single_opens = false;
                    } else {
                        out.push('’');
                        self.next_single_opens = true;
                    }
                    i += 1;
                }
                _ => {
                    out.push(c);
                    i += 1;
                }
            }
        }

        out
    }
}

fn starts_with(chars: &[char], pattern: &str) -> bool {
    pattern
        .chars()
        .enumerate()
        .all(|(offset, expected)| chars.get(offset) == Some(&expected))
}

fn previous(chars: &[char], index: usize) -> Option<char> {
    index.checked_sub(1).and_then(|i| chars.get(i)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str) -> String {
        Converter::new(true).apply(text)
    }

    #[test]
    fn ellipsis_and_dashes() {
        assert_eq!(apply("等一下..."), "等一下…");
        assert_eq!(apply("2010--2020"), "2010–2020");
        assert_eq!(apply("他说---真的"), "他说—真的");
        // `---` 先于 `--` 匹配
        assert_eq!(apply("----"), "—-");
    }

    #[test]
    fn double_quotes_alternate_open_and_close() {
        assert_eq!(apply("\"你好\""), "“你好”");
        // 中文里引号前面直接跟汉字，也要判成开引号
        assert_eq!(apply("他说\"你好\"。"), "他说“你好”。");
        assert_eq!(apply("\"甲\" 和 \"乙\""), "“甲” 和 “乙”");
    }

    #[test]
    fn apostrophes_are_not_opening_quotes() {
        assert_eq!(apply("don't"), "don’t");
        assert_eq!(apply("'单引号'"), "‘单引号’");
    }

    #[test]
    fn quote_state_persists_across_segments() {
        // 扫描器会按分隔符把文本切段，引号状态必须跨段保留
        let mut converter = Converter::new(true);
        assert_eq!(converter.apply("他说\""), "他说“");
        assert_eq!(converter.apply("你好\""), "你好”");
    }

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(
            apply("普通文本，没有特殊符号。"),
            "普通文本，没有特殊符号。"
        );
        assert_eq!(apply("a-b"), "a-b");
    }
}
