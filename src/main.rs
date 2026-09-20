//! `neomark` 命令行：把一篇 neomark 文档编译成一个自包含的 HTML 文件。
//!
//! ```text
//! neomark 文档.nm              # 写出 文档.html（内嵌默认 CSS）
//! neomark -o out.html 文档.nm
//! cat 文档.nm | neomark -      # 写到标准输出
//! ```

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use neomark::{Context, Dispatcher, Registry, html, parse};

const USAGE: &str = "\
neomark — 把 neomark 文档编译成 HTML

用法：
    neomark [选项] [输入文件]

输入文件写 `-` 或省略时，从标准输入读取。

选项：
    -o, --output <文件>   输出路径
                          （默认：输入文件的 .html；读标准输入时写标准输出）
    -t, --title <标题>    页面标题（默认：输入文件名）
        --fragment        只输出 HTML 片段，不加外壳与默认 CSS
    -h, --help            显示本帮助
";

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    let args = match parse_args(&raw) {
        Ok(Some(args)) => args,
        // `--help`
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("neomark: {message}");
            eprint!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("neomark: {message}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Default)]
struct Args {
    input: Option<String>,
    output: Option<PathBuf>,
    title: Option<String>,
    fragment: bool,
}

/// 解析命令行；`Ok(None)` 表示用户要的是帮助。
fn parse_args(raw: &[String]) -> Result<Option<Args>, String> {
    let mut args = Args::default();
    let mut iter = raw.iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "-o" | "--output" => {
                let value = iter.next().ok_or("--output 需要一个参数")?;
                args.output = Some(PathBuf::from(value));
            }
            "-t" | "--title" => {
                let value = iter.next().ok_or("--title 需要一个参数")?;
                args.title = Some(value.clone());
            }
            "--fragment" => args.fragment = true,
            // 其余参数都是输入文件；`-` 是标准输入的约定写法。
            "--" => {
                for rest in iter.by_ref() {
                    set_input(&mut args, rest)?;
                }
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("未知选项：{other}"));
            }
            other => set_input(&mut args, other)?,
        }
    }

    Ok(Some(args))
}

fn set_input(args: &mut Args, value: &str) -> Result<(), String> {
    if args.input.is_some() {
        return Err("只能指定一个输入文件".to_string());
    }
    args.input = Some(value.to_string());
    Ok(())
}

fn run(args: &Args) -> Result<(), String> {
    let from_stdin = matches!(args.input.as_deref(), None | Some("-"));

    let (source, default_title, default_output) = if from_stdin {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| format!("读取标准输入失败：{error}"))?;
        (source, "neomark".to_string(), None)
    } else {
        let path = args.input.as_deref().expect("已经排除标准输入");
        let source =
            fs::read_to_string(path).map_err(|error| format!("读取 {path} 失败：{error}"))?;
        let title = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("neomark")
            .to_string();
        (
            source,
            title,
            Some(PathBuf::from(path).with_extension("html")),
        )
    };

    let mut ast = parse(&source);
    let mut registry = Registry::new();
    neomark::register_defaults(&mut registry);
    let mut ctx = Context::new(&source);
    Dispatcher::new(registry).run(&mut ast, &mut ctx);

    let title = args.title.as_deref().unwrap_or(&default_title);
    let output = if args.fragment {
        html::render(&ast)
    } else {
        html::render_page(&ast, title)
    };

    match args.output.clone().or(default_output) {
        Some(path) => {
            fs::write(&path, &output)
                .map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
            eprintln!("neomark: 已写出 {}", path.display());
            Ok(())
        }
        None => io::stdout()
            .write_all(output.as_bytes())
            .map_err(|error| format!("写标准输出失败：{error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &[&str]) -> Result<Option<Args>, String> {
        let owned: Vec<String> = raw.iter().map(|arg| arg.to_string()).collect();
        parse_args(&owned)
    }

    #[test]
    fn input_and_options_are_read() {
        let args = parse(&["-o", "out.html", "-t", "标题", "doc.nm"])
            .unwrap()
            .unwrap();

        assert_eq!(args.input.as_deref(), Some("doc.nm"));
        assert_eq!(args.output, Some(PathBuf::from("out.html")));
        assert_eq!(args.title.as_deref(), Some("标题"));
        assert!(!args.fragment);
    }

    #[test]
    fn stdin_dash_is_an_input_not_an_option() {
        let args = parse(&["-"]).unwrap().unwrap();
        assert_eq!(args.input.as_deref(), Some("-"));
    }

    #[test]
    fn help_short_circuits() {
        assert!(parse(&["--help"]).unwrap().is_none());
    }

    #[test]
    fn unknown_options_and_missing_values_are_errors() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["-o"]).is_err());
        assert!(parse(&["a.nm", "b.nm"]).is_err());
    }
}
