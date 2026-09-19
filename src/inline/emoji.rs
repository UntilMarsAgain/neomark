//! Emoji 短码 `:name:`。
//!
//! 与实体表同样的取舍：**常用短码子集**而不是 GitHub 那 1800+ 个全量表。
//! 查不到的短码原样保留，不会被吃掉。
//!
//! 只认「小写字母 / 数字 / `_` `+` `-`」组成、且两侧都有冒号的名字，所以
//! 正文里的 `12:30:45` 不会被误判——`:30:` 查不到，原样输出。

/// 解析 `:` 开头的一段。
///
/// 成功时返回（emoji 文本, 吃掉的字符数），字符数包含两侧的冒号。
pub(crate) fn parse(chars: &[char]) -> Option<(String, usize)> {
    if chars.first() != Some(&':') {
        return None;
    }

    // 从开头的冒号之后找收尾的冒号；短码名不可能太长。
    let end = 1 + chars[1..].iter().take(48).position(|&c| c == ':')?;
    if end < 3 {
        // 名字至少两个字符，顺带挡掉 `::`。
        return None;
    }

    let name: String = chars[1..end].iter().collect();
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'+' | b'-'))
    {
        return None;
    }

    lookup(&name).map(|emoji| (emoji.to_string(), end + 1))
}

/// 常用短码表（沿用 GitHub 的短码名）。
fn lookup(name: &str) -> Option<&'static str> {
    Some(match name {
        // 表情
        "smile" => "😄",
        "smiley" => "😃",
        "grin" => "😁",
        "laughing" | "satisfied" => "😆",
        "sweat_smile" => "😅",
        "joy" => "😂",
        "rofl" => "🤣",
        "wink" => "😉",
        "blush" => "😊",
        "heart_eyes" => "😍",
        "kissing_heart" => "😘",
        "thinking" => "🤔",
        "neutral_face" => "😐",
        "expressionless" => "😑",
        "unamused" => "😒",
        "sweat" => "😓",
        "pensive" => "😔",
        "confused" => "😕",
        "upside_down_face" => "🙃",
        "smirk" => "😏",
        "sunglasses" => "😎",
        "cry" => "😢",
        "sob" => "😭",
        "angry" => "😠",
        "rage" => "😡",
        "scream" => "😱",
        "fearful" => "😨",
        "sleeping" => "😴",
        "mask" => "😷",
        "nauseated_face" => "🤢",
        "poop" | "hankey" => "💩",
        "skull" => "💀",
        "ghost" => "👻",
        "alien" => "👽",
        "robot" => "🤖",
        "see_no_evil" => "🙈",
        "hear_no_evil" => "🙉",
        "speak_no_evil" => "🙊",
        // 手势与人物
        "wave" => "👋",
        "thumbsup" | "+1" => "👍",
        "thumbsdown" | "-1" => "👎",
        "ok_hand" => "👌",
        "clap" => "👏",
        "pray" | "praying_hands" => "🙏",
        "muscle" => "💪",
        "point_right" => "👉",
        "point_left" => "👈",
        "point_up" => "☝️",
        "raised_hand" => "✋",
        "eyes" => "👀",
        "brain" => "🧠",
        "baby" => "👶",
        "boy" => "👦",
        "girl" => "👧",
        "man" => "👨",
        "woman" => "👩",
        "bow" => "🙇",
        "shrug" => "🤷",
        "facepalm" => "🤦",
        // 心与符号
        "heart" => "❤️",
        "broken_heart" => "💔",
        "yellow_heart" => "💛",
        "green_heart" => "💚",
        "blue_heart" => "💙",
        "purple_heart" => "💜",
        "black_heart" => "🖤",
        "sparkling_heart" => "💖",
        "star" => "⭐",
        "star2" => "🌟",
        "sparkles" => "✨",
        "zap" => "⚡",
        "boom" | "collision" => "💥",
        "fire" => "🔥",
        "snowflake" => "❄️",
        "rainbow" => "🌈",
        "sunny" => "☀️",
        "cloud" => "☁️",
        "droplet" => "💧",
        "ocean" => "🌊",
        "tada" => "🎉",
        "confetti_ball" => "🎊",
        "gift" => "🎁",
        "balloon" => "🎈",
        "trophy" => "🏆",
        "medal_sports" => "🏅",
        "crown" => "👑",
        "gem" => "💎",
        "moneybag" => "💰",
        "dollar" => "💵",
        "bulb" => "💡",
        "bell" => "🔔",
        "mega" => "📣",
        "loudspeaker" => "📢",
        "mag" => "🔍",
        "key" => "🔑",
        "lock" => "🔒",
        "unlock" => "🔓",
        "shield" => "🛡️",
        "hammer" => "🔨",
        "wrench" => "🔧",
        "gear" => "⚙️",
        "toolbox" => "🧰",
        "link" => "🔗",
        "paperclip" => "📎",
        "pushpin" => "📌",
        "round_pushpin" => "📍",
        "clipboard" => "📋",
        "memo" | "pencil" => "📝",
        "book" => "📕",
        "books" => "📚",
        "notebook" => "📓",
        "bookmark" => "🔖",
        "newspaper" => "📰",
        "email" | "envelope" => "✉️",
        "inbox_tray" => "📥",
        "outbox_tray" => "📤",
        "package" => "📦",
        "file_folder" => "📁",
        "open_file_folder" => "📂",
        "page_facing_up" => "📄",
        "calendar" => "📅",
        "date" => "📆",
        "clock1" | "clock" => "🕐",
        "hourglass" => "⌛",
        "alarm_clock" => "⏰",
        "computer" => "💻",
        "desktop_computer" => "🖥️",
        "keyboard" => "⌨️",
        "iphone" => "📱",
        "camera" => "📷",
        "video_camera" => "📹",
        "tv" => "📺",
        "battery" => "🔋",
        "floppy_disk" => "💾",
        "cd" => "💿",
        "satellite" => "📡",
        "wastebasket" => "🗑️",
        "rocket" => "🚀",
        "airplane" => "✈️",
        "car" => "🚗",
        "bus" => "🚌",
        "train" => "🚆",
        "ship" => "🚢",
        "bike" => "🚲",
        "walking" => "🚶",
        "runner" => "🏃",
        // 图表与状态
        "check" | "heavy_check_mark" | "white_check_mark" => "✅",
        "x" | "heavy_multiplication_x" => "❌",
        "o" | "heavy_circle" => "⭕",
        "warning" => "⚠️",
        "no_entry" => "⛔",
        "no_entry_sign" => "🚫",
        "question" => "❓",
        "exclamation" => "❗",
        "information_source" => "ℹ️",
        "bulb_outline" => "💡",
        "bangbang" => "‼️",
        "interrobang" => "⁉️",
        "chart" | "bar_chart" => "📊",
        "chart_with_upwards_trend" | "trending_up" => "📈",
        "chart_with_downwards_trend" | "trending_down" => "📉",
        "crystal_ball" => "🔮",
        "recycle" => "♻️",
        "white_circle" => "⚪",
        "black_circle" => "⚫",
        "red_circle" => "🔴",
        "large_blue_circle" => "🔵",
        "small_blue_diamond" => "🔹",
        "small_orange_diamond" => "🔸",
        "arrow_right" => "➡️",
        "arrow_left" => "⬅️",
        "arrow_up" => "⬆️",
        "arrow_down" => "⬇️",
        "recycle_bin" => "🗑️",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(text: &str) -> Option<(String, usize)> {
        parse(&text.chars().collect::<Vec<_>>())
    }

    #[test]
    fn common_shortcodes() {
        assert_eq!(parse_str(":smile:").unwrap().0, "😄");
        assert_eq!(parse_str(":thumbsup:").unwrap().0, "👍");
        assert_eq!(parse_str(":rocket:").unwrap().0, "🚀");
        assert_eq!(parse_str(":smile:x").unwrap().1, 7);
    }

    #[test]
    fn plus_minus_and_digits_in_names() {
        assert_eq!(parse_str(":+1:").unwrap().0, "👍");
        assert_eq!(parse_str(":-1:").unwrap().0, "👎");
    }

    #[test]
    fn unknown_shortcodes_and_lookalikes_are_left_alone() {
        assert!(parse_str(":nope:").is_none());
        assert!(parse_str(":30:").is_none());
        assert!(parse_str(":Smile:").is_none()); // 大写不认
        assert!(parse_str(":smile").is_none()); // 缺右冒号
        assert!(parse_str(":").is_none());
        // `::` 是块标记，不该被当成空短码
        assert!(parse_str("::x::").is_none());
    }
}
