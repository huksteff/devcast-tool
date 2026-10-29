use crate::test_util::Recordings;

#[test]
fn ls_shows_objects_in_output_time_sorted_and_filtered() {
    let f = Recordings::new();
    f.ok("zoom add --at 12 --duration 3 --scale 2");
    f.ok("zoom add --at 2 --duration 1");
    f.ok("cut 0-1");
    assert_eq!(
        f.ok("ls zooms --fields scale"),
        "z2  0:01.000–0:02.000  scale=1.5\nz1  0:11.000–0:14.000  scale=2.0"
    );
    assert_eq!(
        f.ok("ls zooms --from 5 --fields mode"),
        "z1  0:11.000–0:14.000  mode=follow"
    );
    assert_eq!(
        f.ok("ls zooms --limit 1 --fields scale"),
        "z2  0:01.000–0:02.000  scale=1.5\nпоказано 1–1 из 2; дальше: devcast ls zooms --offset 1"
    );
    assert_eq!(f.ok("ls zooms --to 1"), "zooms: пусто (всего 0)");
    f.ok("cut 1-2");
    assert!(f.ok("ls zooms").starts_with("z1  0:10.000–0:13.000"));
    assert!(
        f.ok("ls zooms")
            .ends_with("z2  вырезан  focus.x=0.5  focus.y=0.5  mode=follow  scale=1.5"),
        "cut out objects stay listed, last"
    );
}

#[test]
fn ls_clips_and_info_summarise_the_project() {
    let f = Recordings::new();
    f.ok("cut 0-1");
    f.ok("zoom add --at 20 --duration 2");
    assert_eq!(
        f.ok("ls clips"),
        "c1  0:00.000–0:19.000  cuts=1  kind=recording  media=media/a.mp4  speeds=0  trim=0:00.000–0:20.000\n\
         c2  0:19.000–0:29.000  cuts=0  kind=recording  media=media/b.mp4  speeds=0  trim=0:00.000–0:10.000"
    );
    assert_eq!(
        f.ok("info"),
        "Урок 2.dcproj: длительность 0:29.000, всё сохранено\nсписки: annots 0, callouts 0, captions 0, clips 2, zooms 1; подробно: devcast ls <список>"
    );
    let json: serde_json::Value = serde_json::from_str(&f.ok("ls zooms --json")).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["items"][0]["from"], 20.0);
    assert_eq!(json["items"][0]["media"], serde_json::Value::Null);
}

#[test]
fn ls_errors_name_what_exists() {
    let f = Recordings::new();
    let out = f.run("ls zoomz");
    assert_eq!(out.code, 2);
    assert_eq!(
        out.stderr,
        "нет списка «zoomz»; есть: clips, zooms, callouts, captions, annots, sources\n"
    );
    f.ok("zoom add --at 1 --duration 1");
    let out = f.run("ls zooms --fields scal");
    assert_eq!(out.code, 2);
    assert_eq!(out.stderr, "нет поля «scal»; есть: focus, mode, scale\n");
    assert_eq!(f.run("ls zooms --from soon").code, 2);
}
