//! # neomark
//!
//! neomark 是一种“一切皆块”的标记语言。块分两类：
//!
//! * **自然块**：由书写自然产生，空行是切分标志；
//! * **调用块**：由 `::name 参数...` 调用模板/脚本产生，缩进界定块体。
//!
//! ## 模块结构
//!
//! * [`ast`]：块的数据形状，是渲染器唯一依赖的稳定层；
//! * `parse`：解析器实现，内部模块不对外暴露，只通过 crate 根重新导出入口。
//!
//! 依赖方向固定为 `parse → ast`、`html → ast`；`ast` 永不依赖 `parse`。
//!
//! ## 用法
//!
//! ```
//! use neomark::parse_blocks;
//!
//! let blocks = parse_blocks("::notice type=warning:\n  小心！");
//! ```
//!
//! ## 切分规则
//!
//! 1. 任何一行（忽略前导缩进）以 `::` 开头，就是调用块的头部行，
//!    即使它前面没有空行。
//! 2. 头部行之后，缩进**严格大于**头部行缩进的行属于该调用块的块体。
//! 3. 缩进一旦不再严格大于，该行就是下一个块的首行，末尾无需空行。
//! 4. 调用块体内的空行不结束块（只要后面还有更深缩进的行），
//!    末尾的连续空行不算内容。
//! 5. 块体去掉公共缩进后递归解析，因此调用块可以嵌套。

pub mod ast;

mod parse;

pub use ast::{Block, CallBlock, NaturalBlock, Params};
pub use parse::{CallHeader, parse_blocks, parse_call_header};
