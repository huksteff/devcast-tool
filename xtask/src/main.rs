//! Project checks (verify) and maintenance commands: `cargo xtask <command>`.

mod changed;
mod crates;
mod deps;
mod features;
mod hook;
mod limits;
mod metadata;
mod native;
mod run;
mod sources;
mod todo;
mod verify;

use std::process::ExitCode;

const USAGE: &str = "\
Использование: cargo xtask <команда>

Команды:
  verify [--full] [--include-ignored]
      Проверки перед «готово»: fmt, clippy, длина файлов, CLAUDE.md и lints крейтов, зависимости,
      пометки о недоделках, тесты.
      Без --full тесты идут только для крейтов с незакоммиченными правками
      и для крейтов, которые от них зависят. --include-ignored добавляет долгие тесты.
  hook-stop
      Stop-хук Claude Code (.claude/settings.json): если менялся код и verify красный,
      не даёт сессии закончиться и передаёт отчёт агенту.
  deps
      Скачать и проверить по SHA-256 нативные зависимости сборки (FFmpeg, libclang) в .deps/
      и скопировать DLL FFmpeg рядом с собираемыми exe.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    };
    match command.as_str() {
        "verify" => match verify::Options::parse(rest) {
            Ok(options) => {
                let report = verify::run(&options);
                println!("{}", report.render());
                report.exit_code()
            }
            Err(message) => usage_error(&message),
        },
        "hook-stop" => hook::stop(),
        "deps" => {
            // xtask lives in <root>/xtask, so the workspace root is its parent.
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent();
            match root.map(native::run) {
                Some(Ok(())) => ExitCode::SUCCESS,
                Some(Err(message)) => {
                    eprintln!("deps: {message}");
                    ExitCode::FAILURE
                }
                None => usage_error("не найден корень workspace"),
            }
        }
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        other => usage_error(&format!("неизвестная команда `{other}`")),
    }
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("xtask: {message}\n\n{USAGE}");
    ExitCode::from(2)
}
