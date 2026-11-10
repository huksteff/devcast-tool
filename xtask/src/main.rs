//! Project checks (verify) and maintenance commands: `cargo xtask <command>`.

mod changed;
mod crates;
mod deps;
mod features;
mod fonts;
mod golden;
mod hook;
mod limits;
mod metadata;
mod native;
mod package;
mod release;
mod run;
mod schema_docs;
mod seams;
mod sources;
mod todo;
mod verify;

use std::process::ExitCode;

const USAGE: &str = "\
Использование: cargo xtask <команда>

Команды:
  verify [--full] [--include-ignored] [--quick] [--checks | --tests]
      Проверки перед «готово»: fmt, clippy, длина файлов, CLAUDE.md и lints крейтов, зависимости,
      пометки о недоделках, тесты.
      Без --full тесты идут только для крейтов с незакоммиченными правками
      и для крейтов, которые от них зависят. --include-ignored добавляет долгие тесты.
      --quick — только проверки без сборки (без clippy и тестов), секунды.
      --checks — всё, кроме тестов; --tests — только тесты (CI гоняет их на двух машинах сразу).
  hook-stop
      Stop-хук Claude Code (.claude/settings.json): если менялся код и verify --quick красный,
      не даёт сессии закончиться и передаёт отчёт агенту.
  golden update \"<почему>\"
      Перезаписать эталонные кадры (тесты golden с UPDATE_GOLDEN=1) и показать, что изменилось.
      Причина обязательна: она идёт в сообщение коммита.
  deps
      Скачать и проверить по SHA-256 нативные зависимости сборки (FFmpeg, libclang) и шрифты в .deps/
      и скопировать DLL FFmpeg рядом с собираемыми exe.
  package [--version 1.2.3-beta.4] [--channel beta]
      Установщик Velopack: deps, release-сборка devcast-app и devcast, папка пакета и `vpk pack`.
      Результат — в <папка сборки>/velopack/releases. Нужен vpk (docs/SETUP.md).
      Без флагов — версия из Cargo.toml в обычном канале; бету собирает CI (CONTRIBUTING.md).";

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
        "golden" => match golden::run(rest) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("golden: {message}");
                ExitCode::FAILURE
            }
        },
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
        "package" => {
            let options = match release::Options::parse(rest) {
                Ok(options) => options,
                Err(message) => return usage_error(&message),
            };
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent();
            match root.map(|root| package::run(root, &options)) {
                Some(Ok(())) => ExitCode::SUCCESS,
                Some(Err(message)) => {
                    eprintln!("package: {message}");
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
