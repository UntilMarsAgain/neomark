//! 行内扫描：把文本切成**已定型的片段**与**尚未配对的分隔符游程**。
//!
//! 这一步把所有「绑定更紧」的构造先吃掉——转义、硬换行、代码跨度、数学、
//! 实体、emoji——剩下的普通字符累积成文本，而 `*` `~` `^` `=` 的游程留到
//! [`super::delim`] 里配对。这样代码跨度和数学的内部天然不可能触发强调。
//!
//! 注意这里**不做任何渲染决定**：代码跨度只标成 `Code`、数学只标成 `Math`、
//! 短码只记名字，长什么样是 [`crate::html`] 的事。

use super::{Piece, emoji, entities, smart};
use crate::ast::NodeKind;

/// 会成为分隔符的字符。
fn is_delimiter(c: char) -> bool {
    matches!(c, '*' | '~' | '^' | '=')
}

pub(crate) fn scan(text: &str) -> Vec<Piece> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut literal = String::new();
    // 引号状态要跨段保留，所以整段扫描共用一个转换器。
    let mut smart = smart::Converter::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        match c {
            // 转义与硬换行
            '\\' => match chars.get(i + 1).copied() {
                Some('\n') => {
                    flush(&mut out, &mut literal, &mut smart);
                    out.push(Piece::LineBreak);
                    i += 2;
                }
                Some(next) if next.is_ascii_punctuation() => {
                    literal.push(next);
                    i += 2;
                }
                // 不是转义对象也不是换行：反斜杠原样保留
                _ => {
                    literal.push('\\');
                    i += 1;
                }
            },

            // 代码跨度：N 个反引号配对 N 个，内部不做任何解析
            '`' => {
                let (count, after_open) = run_len(&chars, i);
                match find_closer(&chars, after_open, '`', count) {
                    Some(close) => {
                        flush(&mut out, &mut literal, &mut smart);
                        out.push(Piece::Node {
                            kind: NodeKind::Code,
                            children: vec![Piece::Text(span_text(&chars[after_open..close]))],
                        });
                        i = close + count;
                    }
                    None => {
                        push_run(&mut literal, '`', count);
                        i = after_open;
                    }
                }
            }

            // 数学：$ 的配对行为与反引号完全一致
            '$' => {
                let (count, after_open) = run_len(&chars, i);
                match find_closer(&chars, after_open, '$', count) {
                    Some(close) => {
                        flush(&mut out, &mut literal, &mut smart);
                        // 原样存内容。要不要包成 `\(...\)`、用什么标签，
                        // 都是渲染器的决定。
                        out.push(Piece::Node {
                            kind: NodeKind::Math,
                            children: vec![Piece::Text(span_text(&chars[after_open..close]))],
                        });
                        i = close + count;
                    }
                    None => {
                        push_run(&mut literal, '$', count);
                        i = after_open;
                    }
                }
            }

            '&' => match entities::parse(&chars[i..]) {
                Some((value, used)) => {
                    literal.push_str(&value);
                    i += used;
                }
                None => {
                    literal.push('&');
                    i += 1;
                }
            },

            ':' => match emoji::parse(&chars[i..]) {
                Some((name, used)) => {
                    // 短码单独成型：只带走名字，认不认识由渲染器决定。
                    flush(&mut out, &mut literal, &mut smart);
                    out.push(Piece::Emoji(name));
                    i += used;
                }
                None => {
                    literal.push(':');
                    i += 1;
                }
            },

            c if is_delimiter(c) => {
                let (count, after_run) = run_len(&chars, i);
                // 单个 `=` 不成标记。
                if c == '=' && count < 2 {
                    push_run(&mut literal, '=', count);
                    i = after_run;
                    continue;
                }
                flush(&mut out, &mut literal, &mut smart);
                let (can_open, can_close) =
                    flanking(previous(&chars, i), chars.get(after_run).copied());
                out.push(Piece::Delim {
                    ch: c,
                    count,
                    can_open,
                    can_close,
                });
                i = after_run;
            }

            _ => {
                literal.push(c);
                i += 1;
            }
        }
    }

    flush(&mut out, &mut literal, &mut smart);
    out
}

/// 把累积的普通文本收成一个片段，顺便做智能标点。
fn flush(out: &mut Vec<Piece>, literal: &mut String, smart: &mut smart::Converter) {
    if !literal.is_empty() {
        out.push(Piece::Text(smart.apply(literal)));
        literal.clear();
    }
}

/// 从 `start` 开始的、由 `ch` 组成的游程长度，以及游程结束后的下标。
fn run_len(chars: &[char], start: usize) -> (usize, usize) {
    let mut end = start;
    while end < chars.len() && chars[end] == chars[start] {
        end += 1;
    }
    (end - start, end)
}

/// 找到下一个长度**恰好等于** `count` 的同类游程。长度不同的游程不闭合。
fn find_closer(chars: &[char], from: usize, ch: char, count: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == ch {
            let (len, end) = run_len(chars, i);
            if len == count {
                return Some(i);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    None
}

/// 代码跨度 / 数学的内容归一：换行变空格；首尾同时是空格时各剥一个。
fn span_text(chars: &[char]) -> String {
    let mut text: String = chars
        .iter()
        .map(|&c| if c == '\n' { ' ' } else { c })
        .collect();

    if text.len() >= 2
        && text.starts_with(' ')
        && text.ends_with(' ')
        && text.chars().any(|c| c != ' ')
    {
        text = text[1..text.len() - 1].to_string();
    }

    text
}

fn push_run(literal: &mut String, ch: char, count: usize) {
    for _ in 0..count {
        literal.push(ch);
    }
}

fn previous(chars: &[char], index: usize) -> Option<char> {
    index.checked_sub(1).and_then(|i| chars.get(i)).copied()
}

/// CommonMark 的 left/right-flanking 判定。
///
/// 标点判定用 ASCII 标点近似（CommonMark 规定的是 Unicode `P*` 类别）——
/// 对 `*` 的实际影响极小，而引入 Unicode 类别表不值得。
fn flanking(prev: Option<char>, next: Option<char>) -> (bool, bool) {
    let prev_space = prev.is_none_or(char::is_whitespace);
    let next_space = next.is_none_or(char::is_whitespace);
    let prev_punct = prev.is_some_and(|c| c.is_ascii_punctuation());
    let next_punct = next.is_some_and(|c| c.is_ascii_punctuation());

    let left = !next_space && (!next_punct || prev_space || prev_punct);
    let right = !prev_space && (!prev_punct || next_space || next_punct);

    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_util::kind_name;

    fn texts(pieces: &[Piece]) -> Vec<String> {
        pieces
            .iter()
            .map(|piece| match piece {
                Piece::Text(text) => format!("T({text})"),
                Piece::Node { kind, children } => {
                    format!("[{} {}]", kind_name(kind), texts(children).join(""))
                }
                Piece::Emoji(name) => format!("E({name})"),
                Piece::LineBreak => "<br>".to_string(),
                Piece::Delim { ch, count, .. } => format!("D({ch}{count})"),
            })
            .collect()
    }

    fn show(text: &str) -> String {
        texts(&scan(text)).join("")
    }

    #[test]
    fn backslash_escapes() {
        assert_eq!(show("\\*不是强调\\*"), "T(*不是强调*)");
        // 非标点不构成转义，反斜杠保留
        assert_eq!(show("\\中"), "T(\\中)");
    }

    #[test]
    fn backslash_at_end_of_line_is_a_hard_break() {
        assert_eq!(show("上\\\n下"), "T(上)<br>T(下)");
        // 普通换行是软换行，留在文本里
        assert_eq!(show("上\n下"), "T(上\n下)");
    }

    #[test]
    fn code_spans_bind_tightest() {
        assert_eq!(show("`*x*`"), "[code T(*x*)]");
        // 更多的反引号可以包裹内部的反引号
        assert_eq!(show("``a`b``"), "[code T(a`b)]");
        // 长度不同不闭合，原样落回
        assert_eq!(show("``a`"), "T(``a`)");
        // 首尾各一个空格要剥掉
        assert_eq!(show("` a `"), "[code T(a)]");
        assert_eq!(show("`  `"), "[code T(  )]");
    }

    #[test]
    fn math_is_marked_as_math_with_raw_content() {
        assert_eq!(show("$x^2$"), "[math T(x^2)]");
        // $$ 可以包裹内部的 $
        assert_eq!(show("$$a$b$$"), "[math T(a$b)]");
        assert_eq!(show("$x"), "T($x)");
    }

    #[test]
    fn emoji_becomes_a_name_only_piece() {
        // 值不在这里解析——只带走名字。
        assert_eq!(show("&amp; :smile:"), "T(& )E(smile)");
        assert_eq!(show(":nope:"), "E(nope)");
    }

    #[test]
    fn entities_emoji_and_markers_do_not_leak_into_code() {
        assert_eq!(show("`&amp;`"), "[code T(&amp;)]");
        assert_eq!(show("`:smile:`"), "[code T(:smile:)]");
        assert_eq!(show("`*x*`"), "[code T(*x*)]");
    }

    #[test]
    fn delimiter_runs_are_left_for_the_matcher() {
        assert_eq!(show("*x*"), "D(*1)T(x)D(*1)");
        assert_eq!(show("**x**"), "D(*2)T(x)D(*2)");
        assert_eq!(show("~~x~~"), "D(~2)T(x)D(~2)");
        assert_eq!(show("~x~"), "D(~1)T(x)D(~1)");
        assert_eq!(show("==x=="), "D(=2)T(x)D(=2)");
        // 单个 = 不是标记
        assert_eq!(show("a=b"), "T(a=b)");
    }

    #[test]
    fn smart_punctuation_runs_on_plain_text_only() {
        assert_eq!(show("等一下..."), "T(等一下…)");
        assert_eq!(show("`...`"), "[code T(...)]");
    }
}
