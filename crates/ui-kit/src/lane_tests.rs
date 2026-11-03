use dc_core::project::ClipKind;
use dc_core::{ClipId, IdKind};

use crate::lane::Item;

#[test]
fn an_item_is_its_devcast_id() {
    let item = Item::of(ClipId::from_number(12));
    assert_eq!(item.to_string(), "c12");
    assert_eq!(item.id::<ClipKind>(), Some(ClipId::from_number(12)));
    assert_eq!(Item::parse("c12", ["z", ClipKind::PREFIX]), Some(item));
}

#[test]
fn parse_takes_only_canonical_ids_of_known_kinds() {
    for text in ["c", "c012", "c1x", "z3", "12"] {
        assert_eq!(Item::parse(text, [ClipKind::PREFIX]), None, "{text}");
    }
}
