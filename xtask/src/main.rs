//! Project checks (verify) and maintenance commands: `cargo xtask <command>`.

mod changed;
mod deps;
mod limits;
mod metadata;
mod run;
mod sources;
mod todo;
mod verify;

use std::process::ExitCode;

const USAGE: &str = "\
Использование: cargo xtask <команда>

Команды:
  verify [--full] [--include-ignored]
      Проверки перед «готово»: fmt, clippy, длина файлов, зависимости, пометки о недоделках, тесты.
      Без --full тесты идут только для крейтов с незакоммиченными правками
      и для крейтов, которые от них зависят. --include-ignored добавляет долгие тесты.";

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
