//! 分隔符配对：把游程收成 `em` / `strong` / `del` / `sub` / `sup` / `mark`。
//!
//! 用的是 CommonMark 的思路——**最近的同类 opener** 加「3 的规则」——但去掉
//! 了 `openers_bottom` 那个只影响病态输入的缓存优化。`_` 不参与（按需求
//! 只保留 `*`），所以也省掉了下划线特有的词内限制。

use super::Piece;
use crate::ast::NodeKind;

/// 一个分隔符游程的快照。
#[derive(Debug, Clone, Copy)]
struct Delim {
    ch: char,
    count: usize,
    can_open: bool,
    can_close: bool,
}

impl Delim {
    fn shortened_to(self, count: usize) -> Piece {
        Piece::Delim {
            ch: self.ch,
            count,
            can_open: self.can_open,
            can_close: self.can_close,
        }
    }
}

pub(crate) fn resolve(mut nodes: Vec<Piece>) -> Vec<Piece> {
    let mut closer = 0;

    while closer < nodes.len() {
        let Some(closer_info) = delim_at(&nodes, closer) else {
            closer += 1;
            continue;
        };
        if !closer_info.can_close {
            closer += 1;
            continue;
        }

        // 向前找最近的、同字符且允许配对的 opener。
        let mut open_index = None;
        let mut k = closer;
        while k > 0 {
            k -= 1;
            if let Some(open_info) = delim_at(&nodes, k)
                && open_info.ch == closer_info.ch
                && open_info.can_open
                && allowed(open_info, closer_info)
            {
                open_index = Some(k);
                break;
            }
        }

        let Some(open_index) = open_index else {
            closer += 1;
            continue;
        };
        let open_info = delim_at(&nodes, open_index).expect("刚刚确认过是分隔符");

        let Some((kind, used)) = matching(open_info.ch, open_info.count, closer_info.count) else {
            closer += 1;
            continue;
        };

        // 中间的内容此时早已处理完，直接搬作孩子。
        let children: Vec<Piece> = nodes.drain(open_index + 1..closer).collect();

        let mut replacement: Vec<Piece> = Vec::with_capacity(3);
        if open_info.count > used {
            replacement.push(open_info.shortened_to(open_info.count - used));
        }
        replacement.push(Piece::Node { kind, children });
        if closer_info.count > used {
            replacement.push(closer_info.shortened_to(closer_info.count - used));
        }

        // 被 drain 之后，closer 已经挪到 open_index + 1。
        nodes.splice(open_index..=open_index + 1, replacement);

        // 剩下的分隔符可能还能继续配对，从 opener 处回头再扫。
        closer = open_index;
    }

    nodes
}

fn delim_at(nodes: &[Piece], index: usize) -> Option<Delim> {
    match nodes.get(index)? {
        Piece::Delim {
            ch,
            count,
            can_open,
            can_close,
        } => Some(Delim {
            ch: *ch,
            count: *count,
            can_open: *can_open,
            can_close: *can_close,
        }),
        _ => None,
    }
}

/// CommonMark 的「3 的规则」，只对 `*` 生效。
fn allowed(opener: Delim, closer: Delim) -> bool {
    if opener.ch != '*' {
        return true;
    }
    !(opener.can_close || closer.can_open)
        || !(opener.count + closer.count).is_multiple_of(3)
        || (opener.count.is_multiple_of(3) && closer.count.is_multiple_of(3))
}

/// 决定配成什么**语义**、以及消耗几个分隔符。这里不提任何 HTML。
fn matching(ch: char, open: usize, close: usize) -> Option<(NodeKind, usize)> {
    match ch {
        '*' => Some(if open >= 2 && close >= 2 {
            (NodeKind::Strong, 2)
        } else {
            (NodeKind::Emphasis, 1)
        }),
        // `~~` 是删除线，单个 `~` 是下标。
        '~' => Some(if open >= 2 && close >= 2 {
            (NodeKind::Strikethrough, 2)
        } else {
            (NodeKind::Subscript, 1)
        }),
        '^' => Some((NodeKind::Superscript, 1)),
        '=' if open >= 2 && close >= 2 => Some((NodeKind::Mark, 2)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_util::kind_name;
    use crate::inline::scan;

    /// 只跑到配对为止的树形打印。
    fn show(text: &str) -> String {
        fn render(pieces: &[Piece]) -> String {
            pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => text.clone(),
                    Piece::Node { kind, children } => {
                        format!("[{} {}]", kind_name(kind), render(children))
                    }
                    Piece::InlineCall { name, .. } => format!("I({name})"),
                    Piece::Link { target, text } => format!("L({text} => {target})"),
                    Piece::LineBreak => "<br>".to_string(),
                    Piece::Delim { ch, count, .. } => format!("{ch}{count}"),
                })
                .collect()
        }
        render(&resolve(scan::scan(text)))
    }

    #[test]
    fn emphasis_and_strong() {
        assert_eq!(show("*a*"), "[emphasis a]");
        assert_eq!(show("**a**"), "[strong a]");
        assert_eq!(show("***a***"), "[emphasis [strong a]]");
        assert_eq!(show("**a *b* c**"), "[strong a [emphasis b] c]");
    }

    #[test]
    fn intraword_asterisks_work_which_is_why_underscores_were_dropped() {
        assert_eq!(show("a*b*c"), "a[emphasis b]c");
    }

    #[test]
    fn unmatched_delimiters_fall_back_to_literal() {
        assert_eq!(show("*a"), "*1a");
        assert_eq!(show("a*"), "a*1");
    }

    #[test]
    fn spaces_inside_prevent_matching() {
        // 右侧翼判定：开头的 * 后面跟空格不能开
        assert_eq!(show("* a *"), "*1 a *1");
    }

    #[test]
    fn strikethrough_subscript_superscript_and_highlight() {
        assert_eq!(show("~~a~~"), "[strikethrough a]");
        assert_eq!(show("~a~"), "[subscript a]");
        assert_eq!(show("2^10^"), "2[superscript 10]");
        assert_eq!(show("==a=="), "[mark a]");
    }

    #[test]
    fn delimiters_do_not_cross_kinds() {
        // `*` 不能被 `~` 闭合
        assert_eq!(show("*a~"), "*1a~1");
    }
}
