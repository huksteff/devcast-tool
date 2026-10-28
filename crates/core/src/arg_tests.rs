use serde_json::{Value, json};

use super::*;
use crate::time::{Output, OutputTime, Range};

fn o(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

fn time(value: Value) -> Result<OutputTime, String> {
    serde_json::from_value::<TimeArg<Output>>(value)
        .map(|arg| arg.0)
        .map_err(|e| e.to_string())
}

fn range(value: Value) -> Result<Range<Output>, String> {
    serde_json::from_value::<RangeArg<Output>>(value)
        .map(|arg| arg.0)
        .map_err(|e| e.to_string())
}

fn r(a: f64, b: f64) -> Range<Output> {
    Range::new(o(a), o(b)).unwrap()
}

#[test]
fn time_from_seconds_or_text() {
    assert_eq!(time(json!(12.5)), Ok(o(12.5)));
    assert_eq!(time(json!(3)), Ok(o(3.0)));
    assert_eq!(time(json!("1:02.5")), Ok(o(62.5)));
    assert_eq!(time(json!("1:00:00")), Ok(o(3600.0)));
    let error = time(json!("12с")).unwrap_err();
    assert!(error.contains("не понимаю время «12с»"), "{error}");
    assert!(time(json!(true)).is_err());
}

#[test]
fn range_from_text() {
    assert_eq!(range(json!("10.2-12.5")), Ok(r(10.2, 12.5)));
    assert_eq!(range(json!(" 1:02 - 1:05.5 ")), Ok(r(62.0, 65.5)));
    assert_eq!(range(json!("3-3")), Ok(r(3.0, 3.0)));
    assert_eq!("0-1".parse::<Range<Output>>(), Ok(r(0.0, 1.0)));
    for bad in ["12", "-1-2", "1-", "a-b", "1-2-3"] {
        let error = range(json!(bad)).unwrap_err();
        assert!(error.contains("диапазон"), "{bad}: {error}");
    }
    let inverted = range(json!("5-2")).unwrap_err();
    assert!(inverted.contains("раньше начала"), "{inverted}");
    assert!(range(json!(12)).is_err());
}

#[test]
fn range_list_from_text() {
    let list = |value| {
        serde_json::from_value::<RangeListArg<Output>>(value)
            .map(|arg| arg.0)
            .map_err(|e| e.to_string())
    };
    assert_eq!(list(json!("1-2,5-6")), Ok(vec![r(1.0, 2.0), r(5.0, 6.0)]));
    assert_eq!(
        list(json!("1-2, 0:05-0:06")),
        Ok(vec![r(1.0, 2.0), r(5.0, 6.0)])
    );
    assert_eq!(list(json!("1-2")), Ok(vec![r(1.0, 2.0)]));
    assert!(list(json!("")).is_err());
    assert!(list(json!("1-2,,3-4")).is_err());
}

#[test]
fn schemas_tell_the_cli_how_to_read_values() {
    let schema = |s: schemars::Schema| s.as_value().clone();
    let time = schema(schemars::schema_for!(TimeArg<Output>));
    assert_eq!(time["type"], json!(["string", "number"]));
    let range = schema(schemars::schema_for!(RangeArg<Output>));
    assert_eq!(range["type"], json!("string"));
    let list = schema(schemars::schema_for!(RangeListArg<Output>));
    assert_eq!(list["type"], json!("string"));
}
