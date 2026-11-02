//! `devcast captions …` on the synthesized speech of `transcript_tests` (words already in the
//! cache, so no model is needed).

use crate::run::{Outcome, run};
use crate::transcript_tests::Speech;

fn run_args(speech: &Speech, args: &[&str]) -> Outcome {
    let args = args.iter().map(|a| (*a).to_owned()).collect();
    run(args, &speech.root, &dc_ops::Registry::devcast().unwrap())
}

#[test]
fn gen_export_translate_import_and_undo() {
    let speech = Speech::new();
    assert_eq!(speech.ok("captions gen"), "реплики +3 s1..s3");
    let listed = speech.ok("ls captions");
    assert_eq!(
        listed,
        "s1  0:00.260–0:00.680  text=Привет!
s2  0:01.800–0:06.630  text=Сегодня я покажу, как записать видео с экрана и вырезать паузы.
s3  0:07.360–0:09.130  text=Это занимает пару минут."
    );
    assert_eq!(
        speech.ok("captions export -o субтитры.txt"),
        "реплик 3 → субтитры.txt"
    );
    let exported = std::fs::read_to_string(speech.root.join("субтитры.txt")).unwrap();
    assert_eq!(
        exported,
        "s1 Привет!\ns2 Сегодня я покажу, как записать видео с экрана и вырезать паузы.\ns3 Это занимает пару минут.\n"
    );
    std::fs::write(
        speech.root.join("перевод.txt"),
        "\u{feff}s1 Hi!\ns2 Today I will show how to record the screen and cut pauses.\n",
    )
    .unwrap();
    assert_eq!(speech.ok("captions import перевод.txt"), "реплики ~2 s1,s2");
    let translated = speech.ok("ls captions");
    assert!(translated.contains("text=Hi!"), "{translated}");
    let times = |list: &str| -> Vec<String> {
        list.lines()
            .map(|l| l.split("text=").next().unwrap().to_owned())
            .collect()
    };
    assert_eq!(times(&translated), times(&listed));
    speech.ok("undo");
    assert_eq!(speech.ok("ls captions"), listed);
}

#[test]
fn edit_cues_from_the_command_line() {
    let speech = Speech::new();
    speech.ok("captions gen");
    assert_eq!(
        speech.ok("captions split s2 --at 4.3"),
        "реплики ~1 s2; реплики +1 s4"
    );
    let out = run_args(
        &speech,
        &[
            "captions",
            "set",
            "s4",
            "--text",
            "видео с экрана и вырежу паузы.",
        ],
    );
    assert_eq!(out.stdout.trim(), "реплики ~1 s4", "{}", out.stderr);
    assert_eq!(
        speech.ok("captions merge s2"),
        "реплики ~1 s2; реплики -1 s4"
    );
    assert_eq!(
        speech.ok("captions set s3 --from 7 --to 9.5"),
        "реплики ~1 s3"
    );
    assert_eq!(speech.ok("captions rm s1"), "реплики -1 s1");
    assert_eq!(
        speech.ok("captions clear --dry-run"),
        "dry-run, проект не изменён: реплики -2 s2,s3"
    );
    let listed = speech.ok("ls captions --fields text");
    assert!(listed.contains("вырежу паузы."), "{listed}");
    assert!(speech.ok("help").contains("captions gen"));
    let help = speech.ok("help captions split");
    assert!(
        help.contains("пример: devcast captions split s3 --at 14.2"),
        "{help}"
    );
}

#[test]
fn cues_stay_on_their_words_after_a_cut() {
    let speech = Speech::new();
    speech.ok("captions gen");
    speech.ok("cut 0-1.5");
    assert_eq!(
        speech.ok("ls captions --fields text"),
        "s2  0:00.300–0:05.130  text=Сегодня я покажу, как записать видео с экрана и вырезать паузы.
s3  0:05.860–0:07.630  text=Это занимает пару минут.
s1  вырезан  text=Привет!"
    );
}

#[test]
fn errors_say_what_to_do() {
    let speech = Speech::new();
    let out = speech.run("captions export -o t.txt");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("captions gen"), "{}", out.stderr);
    let out = speech.run("captions gen --tracks system");
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("нет дорожки «system»"),
        "{}",
        out.stderr
    );
    let out = speech.run("captions import нет.txt");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("captions export"), "{}", out.stderr);
    speech.ok("captions gen");
    let out = speech.run("captions split s9 --at 1");
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("s9 не найден (есть s1..s3)"),
        "{}",
        out.stderr
    );
}
