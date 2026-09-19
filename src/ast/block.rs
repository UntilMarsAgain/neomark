//! 块的类型定义。
//!
//! neomark 的块分为两类：
//!
//! * [`NaturalBlock`]：由书写自然产生的块，用空行切分；
//! * [`CallBlock`]：由 `::name 参数...` 调用模板/脚本产生的块，用缩进界定块体。
//!
//! 调用块的块体会被递归解析，所以子结构仍然是 [`Block`]。

use super::params::Params;

/// 文档中的一个块。
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// 自然块。
    Natural(NaturalBlock),
    /// 调用块。
    Call(CallBlock),
}

/// 自然块：被空行（或调用块行首）切分出来的一段普通文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalBlock {
    /// 块的原文，多行用 `\n` 连接，不含首尾空行与行尾换行。
    pub text: String,
}

/// 调用块：`::name 参数...:` 加上它缩进的块体。
///
/// 块体在去掉公共缩进后被递归解析，因此 `body` 里既可能是自然块，
/// 也可能是嵌套的调用块。
#[derive(Debug, Clone, PartialEq)]
pub struct CallBlock {
    /// 调用名，例如 `notice`。
    pub name: String,
    /// 参数表（键与值都是字符串）。
    pub params: Params,
    /// 递归解析后的块体；没有块体时为空数组。
    pub body: Vec<Block>,
}
