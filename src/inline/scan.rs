//! 行内扫描：把文本切成**已定型的片段**与**尚未配对的分隔符游程**。
//!
//! 这一步把所有「绑定更紧」的构造先吃掉——转义、硬换行、代码跨度、数学、
//! 实体、emoji——剩下的普通字符累积成文本，而 `*` `~` `^` `=` 的游程留到
//! [`super::delim`] 里配对。这样代码跨度和数学的内部天然不可能触发强调。
//!
//! 注意这里**不做任何渲染决定**：代码跨度只标成 `Code`、数学只标成 `Math`、
//! 短码只记名字，长什么样是 [`crate::html`] 的事。

use super::options::Options;
use super::{Piece, entities, smart};
use crate::ast::{NodeKind, Params};

/// 会成为分隔符的字符。
fn is_delimiter(c: char) -> bool {
    matches!(c, '*' | '~' | '^' | '=')
}

pub(crate) fn scan(text: &str, options: Options) -> Vec<Piece> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut literal = String::new();
    // 引号状态要跨段保留，所以整段扫描共用一个转换器。
    let mut smart = smart::Converter::new(options.smart_punctuation);
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        match c {
            // 转义与硬换行
            '\\' if options.escapes => match chars.get(i + 1).copied() {
                Some('\n') => {
                    flush(&mut out, &mut literal, &mut smart);
                    out.push(Piece::LineBreak);
                    i += 2;
                }
                Some(next) if next.is_ascii_punctuation() => {
                    if smart::affects(next) {
                        // 转义的标点必须原样活下来：不能进那个会被智能标点
                        // 转换的缓冲区，否则 `\"` 会变成 `“`，转义就白写了。
                        flush(&mut out, &mut literal, &mut smart);
                        out.push(Piece::Text(next.to_string()));
                    } else {
                        literal.push(next);
                    }
                    i += 2;
                }
                // 不是转义对象也不是换行：反斜杠原样保留
                _ => {
                    literal.push('\\');
                    i += 1;
                }
            },

            // 代码跨度与数学：**同一个分支**，所以「包裹行为一致」是代码本身的
            // 事实，而不是两份拷贝碰巧写得一样。
            //
            // 规则：开头的 N 个定界符必须由**恰好 N 个**闭合（长度不同不闭合，
            // 原样落回文本）；内容里的换行归空格；首尾同时是空格且内容不全是
            // 空格时各剥一个；内部**没有任何转义**，反斜杠也是字面字符。
            '`' | '$' => {
                let (kind, enabled) = if c == '`' {
                    (NodeKind::Code, options.code_spans)
                } else {
                    (NodeKind::Math, options.math)
                };
                if !enabled {
                    literal.push(c);
                    i += 1;
                    continue;
                }

                let (count, after_open) = run_len(&chars, i);
                match find_closer(&chars, after_open, c, count) {
                    Some(close) => {
                        flush(&mut out, &mut literal, &mut smart);
                        // 内容原样存下来：要不要包成 `\(...\)`、用什么标签，
                        // 都是渲染器的决定。
                        out.push(Piece::Node {
                            kind,
                            children: vec![Piece::Text(span_text(&chars[after_open..close]))],
                        });
                        i = close + count;
                    }
                    None => {
                        push_run(&mut literal, c, count);
                        i = after_open;
                    }
                }
            }

            '&' if options.entities => match entities::parse(&chars[i..]) {
                Some((value, used)) => {
                    literal.push_str(&value);
                    i += used;
                }
                None => {
                    literal.push('&');
                    i += 1;
                }
            },

            // 行内调用的糖形态：`:name:`
            ':' if options.calls => match parse_bare_call(&chars, i) {
                Some((name, used)) => {
                    flush(&mut out, &mut literal, &mut smart);
                    out.push(Piece::InlineCall {
                        name,
                        params: Params::new(),
                        content: None,
                    });
                    i += used;
                }
                None => {
                    literal.push(':');
                    i += 1;
                }
            },

            // 行内调用的全形：`{{name k=v: 内容}}`
            '{' if options.calls && chars.get(i + 1) == Some(&'{') => {
                match parse_inline_call(&chars, i) {
                    Some((name, params, content, end)) => {
                        flush(&mut out, &mut literal, &mut smart);
                        out.push(Piece::InlineCall {
                            name,
                            params,
                            content,
                        });
                        i = end;
                    }
                    None => {
                        literal.push('{');
                        i += 1;
                    }
                }
            }

            // 链接：[[ Text => Target ]]
            '[' if options.links && chars.get(i + 1) == Some(&'[') => match parse_link(&chars, i) {
                Some((target, text, end)) => {
                    flush(&mut out, &mut literal, &mut smart);
                    out.push(Piece::Link { target, text });
                    i = end;
                }
                None => {
                    literal.push('[');
                    i += 1;
                }
            },

            c if is_delimiter(c) => {
                let enabled = if c == '*' {
                    options.emphasis
                } else {
                    options.styles
                };
                if !enabled {
                    literal.push(c);
                    i += 1;
                    continue;
                }

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

/// 解析一个链接 `[[ Text => Target ]]`。
///
/// 返回（目标, 文本, 结束下标）。三条规则：
///
/// * 区域由**第一个 `]]`** 界定——这同时就是「不允许链接嵌套」的实现：
///   嵌套链接必须含 `]]`，而区域里不可能含 `]]`。
/// * 多个 `=>` **以最后一个为准**，所以文本里可以出现 `=>`。
/// * 文本与目标各自去掉首尾空白。
///
/// 没有 `=>`、没有收尾 `]]`、或者根本没写成 `[[`，都返回 `None`，
/// 由调用方原样落回字面文本。
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let mut close = None;
    let mut i = start + 2;
    while i + 1 < chars.len() {
        if chars[i] == ']' && chars[i + 1] == ']' {
            close = Some(i);
            break;
        }
        i += 1;
    }
    let close = close?;

    let region: String = chars[start + 2..close].iter().collect();
    // `=>` 是 ASCII，所以字节下标切分是安全的。
    let split = find_arrow(&region)?;

    let text = region[..split].trim().to_string();
    // 目标可以整体用引号包裹（引号内可含空格），引号规则与调用头的键值一致。
    //
    // 文本**不**去引号：它是行内内容，会走行内层，那里的引号属于智能标点
    // 的地盘——`[[他说"你好" => /x]]` 里的引号必须原样交给智能标点。
    let target = crate::parse::unquote_str(region[split + 2..].trim());

    Some((target, text, close + 2))
}

/// 找出分隔箭头的字节下标。
///
/// 从右往左找**第一个目标形态合法的箭头**：目标里出现引号时，它必须是一个完整
/// 的引号串（`"…"`）。于是：
///
/// * 引号保护目标里的箭头：`[[a => "b => c"]]`；
/// * 文本里出现箭头时仍是「最后一个为准」：`[[a => b => /x]]`；
/// * 一个合法目标都找不到时（比如目标里有落单的引号），退回「最后一个箭头」，
///   也就是没有引号规则时的行为——不让新规则把本来能解析的链接弄坏。
///
/// 从右往左而不是从左往右：**文本**部分不参与引号处理（它是行内内容），里面的
/// 引号可能不配对，方向反过来才能在碰到它之前就定下分隔符。
fn find_arrow(region: &str) -> Option<usize> {
    let mut fallback = None;
    let mut search_end = region.len();

    while let Some(at) = region[..search_end].rfind("=>") {
        let raw = region[at + 2..].trim();
        if !raw.contains('"') || is_fully_quoted(raw) {
            return Some(at);
        }
        fallback.get_or_insert(at);
        search_end = at;
    }

    fallback
}

/// `text` 是否是一个完整的引号串：以 `"` 开头，且配对的收尾引号正好在末尾。
fn is_fully_quoted(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars.first() != Some(&'"') {
        return false;
    }

    let mut escaped = false;
    for (index, &c) in chars.iter().enumerate().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => return index == chars.len() - 1,
            _ => {}
        }
    }

    false
}

/// 识别行内调用的**糖形态** `:name:`。
///
/// 只认「小写字母 / 数字 / `_` `+` `-`」组成、两侧都有冒号、名字至少两个字符的
/// 形式，所以正文里的 `12:30:45` 不会被误判。它等价于 `{{name}}`。
fn parse_bare_call(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start) != Some(&':') {
        return None;
    }

    let end = start + 1 + chars[start + 1..].iter().take(48).position(|&c| c == ':')?;
    if end < start + 3 {
        return None;
    }

    let name: String = chars[start + 1..end].iter().collect();
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'+' | b'-'))
    {
        return None;
    }

    Some((name, end + 1 - start))
}

/// 解析行内调用的**全形** `{{name k=v: 内容}}`。
///
/// 返回（名字, 参数, 首行内容, 结束下标）。
///
/// * `}}` 按**配平**匹配，所以内容里可以再嵌 `{{...}}`；不配平就返回 `None`，
///   原样落回字面文本。
/// * 括号内部交给 [`crate::parse::parse_call_header`]——**和块头部是同一个
///   解析器**，这才是「括号内部与块头部逐字节相同」的真正保证，而不是靠
///   约定。
fn parse_inline_call(
    chars: &[char],
    start: usize,
) -> Option<(String, Params, Option<String>, usize)> {
    let mut depth = 1usize;
    let mut i = start + 2;

    let close = loop {
        if i + 1 >= chars.len() {
            return None;
        }
        if chars[i] == '{' && chars[i + 1] == '{' {
            depth += 1;
            i += 2;
        } else if chars[i] == '}' && chars[i + 1] == '}' {
            depth -= 1;
            if depth == 0 {
                break i;
            }
            i += 2;
        } else {
            i += 1;
        }
    };

    let inside: String = chars[start + 2..close].iter().collect();
    let header = crate::parse::parse_call_header(&inside);

    Some((header.name, header.params, header.content, close + 2))
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
                Piece::InlineCall {
                    name,
                    params,
                    content,
                } => {
                    let mut out = format!("I({name}");
                    for (key, value) in params.iter() {
                        out.push_str(&format!(" {key}={value}"));
                    }
                    if let Some(content) = content {
                        out.push_str(&format!(": {content}"));
                    }
                    out.push(')');
                    out
                }
                Piece::Link { target, text } => format!("L({text} => {target})"),
                Piece::LineBreak => "<br>".to_string(),
                Piece::Delim { ch, count, .. } => format!("D({ch}{count})"),
            })
            .collect()
    }

    fn show(text: &str) -> String {
        texts(&scan(text, Options::default())).join("")
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
    fn wrapping_a_lone_backtick_takes_two_backticks_and_two_spaces() {
        // `` ` ``  →  内容是单个反引号。中间那个长度 1 的游程不闭合长度 2 的游程。
        assert_eq!(show("`` ` ``"), "[code T(`)]");
        // 要包住两个反引号就得用三个，依此类推
        assert_eq!(show("``` `` ```"), "[code T(``)]");
        assert_eq!(show("```` ``` ````"), "[code T(```)]");
    }

    #[test]
    fn content_that_is_all_spaces_is_not_trimmed() {
        // 两侧各剥一个空格的前提是「内容不全是空格」
        assert_eq!(show("`` ``"), "[code T( )]");
        assert_eq!(show("``  ``"), "[code T(  )]");
    }

    #[test]
    fn there_are_no_escapes_inside_code() {
        // 按需求：代码跨度内部一律原样，反斜杠也不例外
        assert_eq!(show("`\\`"), "[code T(\\)]");
        assert_eq!(show("`` \\` ``"), "[code T(\\`)]");
        assert_eq!(show("`\\*x\\*`"), "[code T(\\*x\\*)]");
    }

    #[test]
    fn math_wraps_exactly_like_code_spans() {
        // 与代码跨度走**同一个分支**，所以下面逐条与上面的反引号用例对应。
        // 你说的例子：`$$ $ $$` 的内容是一个 `$`。
        assert_eq!(show("$$ $ $$"), "[math T($)]");
        assert_eq!(show("$$$ $$ $$$"), "[math T($$)]");
        // 长度不同不闭合，原样落回
        assert_eq!(show("$$$ $"), "T($$$ $)");
        // 首尾各一个空格要剥掉；内容全是空格则不剥
        assert_eq!(show("$ a $"), "[math T(a)]");
        assert_eq!(show("$$ $$"), "[math T( )]");
        // 内部没有转义
        assert_eq!(show("$\\$"), "[math T(\\)]");
    }

    #[test]
    fn math_is_marked_as_math_with_raw_content() {
        assert_eq!(show("$x^2$"), "[math T(x^2)]");
        // $$ 可以包裹内部的 $
        assert_eq!(show("$$a$b$$"), "[math T(a$b)]");
        assert_eq!(show("$x"), "T($x)");
    }

    #[test]
    fn the_sugar_form_is_an_inline_call_with_no_params() {
        // 糖形态不在这里解析任何值——只带走名字。
        assert_eq!(show("&amp; :smile:"), "T(& )I(smile)");
        assert_eq!(show(":nope:"), "I(nope)");
        assert_eq!(show(":+1:"), "I(+1)");
        // 形状不合法就落回字面
        assert_eq!(show("12:30:45"), "T(12)I(30)T(45)");
        // ↑ `:30:` 形状合法，所以确实是行内调用；渲染器不认识这个名字，
        //   会原样回显成 `:30:`，于是输出仍是 12:30:45（与旧行为一致）。
        assert_eq!(show(":Smile:"), "T(:Smile:)");
    }

    #[test]
    fn the_braced_form_carries_params_and_content() {
        assert_eq!(show("{{smile}}"), "I(smile)");
        assert_eq!(show("{{a b=1}}"), "I(a b=1)");
        assert_eq!(show("{{a b=1: 内容}}"), "I(a b=1: 内容)");
        // 头部语法与块完全相同：值里的冒号不会被误当分隔符
        assert_eq!(show("{{a url=http://x}}"), "I(a url=http://x)");
        // 周围是普通文本
        assert_eq!(show("看 {{a}} 这里"), "T(看 )I(a)T( 这里)");
    }

    #[test]
    fn braces_are_balanced_so_content_may_nest_an_inline_call() {
        assert_eq!(show("{{outer: {{inner}}}}"), "I(outer: {{inner}})");
    }

    #[test]
    fn unbalanced_braces_fall_back_to_literal_text() {
        assert_eq!(show("{{a b=1}} 未闭合 {{c"), "I(a b=1)T( 未闭合 {{c)");
        assert_eq!(show("{{未闭合"), "T({{未闭合)");
    }

    #[test]
    fn inline_calls_do_not_trigger_inside_code_or_math() {
        assert_eq!(show("`{{a}}`"), "[code T({{a}})]");
        assert_eq!(show("`:smile:`"), "[code T(:smile:)]");
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
    fn escapes_are_not_touched_by_smart_punctuation() {
        // 转义的意义就是原样：这一条如果不过，`\"` 会被智能标点改成 `“`
        assert_eq!(show("\\\"a\\\""), "T(\")T(a)T(\")");
        assert_eq!(show("\\-\\-"), "T(-)T(-)");
        assert_eq!(show("don\\'t"), "T(don)T(')T(t)");
        // 没转义就照常转换
        assert_eq!(show("\"a\""), "T(“a”)");
    }

    #[test]
    fn smart_punctuation_runs_on_plain_text_only() {
        assert_eq!(show("等一下..."), "T(等一下…)");
        assert_eq!(show("`...`"), "[code T(...)]");
    }

    #[test]
    fn links_carry_their_text_and_target() {
        assert_eq!(show("[[文本 => 目标]]"), "L(文本 => 目标)");
        // 前后空白被忽略
        assert_eq!(show("[[  文本  =>  目标  ]]"), "L(文本 => 目标)");
        // 周围是普通文本
        assert_eq!(show("看 [[a => b]] 这里"), "T(看 )L(a => b)T( 这里)");
    }

    #[test]
    fn link_targets_may_be_quoted() {
        // 目标里的空格要靠引号包住
        assert_eq!(show("[[文本 => \"a b\"]]"), "L(文本 => a b)");
        // 引号内用 \" 转义
        assert_eq!(show("[[a => \"x\\\"y\"]]"), "L(a => x\"y)");
        // 未闭合的引号按「到目标结尾为止」宽容处理
        assert_eq!(show("[[a => \"b]]"), "L(a => b)");
    }

    #[test]
    fn quotes_protect_arrows_inside_the_target() {
        // 引号内的 `=>` 不是分隔符——和调用头里引号保护 `=` 与 `:` 是同一条规则
        assert_eq!(show("[[a => \"b => c\"]]"), "L(a => b => c)");
        // 但文本里的箭头仍然参与「最后一个为准」
        assert_eq!(show("[[a => b => /x]]"), "L(a => b => /x)");
    }

    #[test]
    fn link_text_keeps_its_quotes_because_it_is_inline_content() {
        // 文本不去引号：那里的引号归智能标点管，必须原样交给行内层
        assert_eq!(show("[[\"q\" => u]]"), "L(\"q\" => u)");
        assert_eq!(show("[[他说\"你好\" => /x]]"), "L(他说\"你好\" => /x)");
    }

    #[test]
    fn an_unbalanced_quote_in_the_text_does_not_break_the_split() {
        // 反向扫描的意义：碰到文本里那个落单的引号之前，分隔符就已经找到了
        assert_eq!(show("[[说\" => /x]]"), "L(说\" => /x)");
    }

    #[test]
    fn the_last_arrow_wins_so_text_may_contain_arrows() {
        assert_eq!(show("[[a => b => c]]"), "L(a => b => c)");
        assert_eq!(show("[[=> 只有目标]]"), "L( => 只有目标)");
    }

    #[test]
    fn malformed_links_fall_back_to_literal_text() {
        assert_eq!(show("[[没有箭头]]"), "T([[没有箭头]])");
        assert_eq!(show("[[a => 没有收尾"), "T([[a => 没有收尾)");
        assert_eq!(show("[不是链接]"), "T([不是链接])");
    }

    #[test]
    fn the_first_closing_bracket_ends_the_region_which_is_why_links_cannot_nest() {
        // 区域由第一个 `]]` 界定 ⇒ 区域里不可能含 `]]` ⇒ 不可能含一个完整链接。
        // 这里 `=> c` 落在区域里，于是最后一个箭头是它。
        assert_eq!(show("[[a [[b => c]] => d]]"), "L(a [[b => c)T( => d]])");
    }

    #[test]
    fn links_do_not_trigger_inside_code_or_math() {
        assert_eq!(show("`[[a => b]]`"), "[code T([[a => b]])]");
        assert_eq!(show("$[[a => b]]$"), "[math T([[a => b]])]");
    }
}
