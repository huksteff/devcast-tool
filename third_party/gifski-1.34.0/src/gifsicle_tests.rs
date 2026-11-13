use super::*;

// The original traversal is safe on these shallow trees and fixes the intended tie order.
fn recursive(lookup: &mut Lookup<'_>, pos: usize, node: NodeId, diff: u64, dither: RgbDiff) {
    let Some(px) = lookup.image.px_at_pos(pos) else { return };
    let children = lookup.code_table.nodes[node as usize].children.clone();
    for child in children {
        if let Some((next, total, dither)) = lookup.try_node(pos, child, px, dither, diff) {
            recursive(lookup, next, child, total, dither);
        }
    }
}

fn branching_table() -> CodeTable {
    let mut table = CodeTable { nodes: Vec::new(), links_used: 0, clear_code: 4 };
    table.reset();
    let mut level: Vec<NodeId> = (0..4).collect();
    for _ in 0..5 {
        let mut next = Vec::new();
        for parent in level {
            for child in 0..3 {
                let id = table.nodes.len() as NodeId;
                table.define(parent, ((parent + child) % 4) as u8, id);
                next.push(id);
            }
        }
        level = next;
    }
    table
}

#[test]
fn iterative_lookup_preserves_recursive_results_and_equal_error_ties() {
    let table = branching_table();
    // Duplicate colors make palette-index ties observable in the selected codeword.
    let pal = [RGB8::new(0, 0, 0), RGB8::new(0, 0, 0), RGB8::new(10, 15, 20), RGB8::new(250, 250, 250)];
    let mut pending = Vec::new();
    for seed in 0u32..32 {
        let mut state = seed;
        let pixels: Vec<u8> = (0..12).map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            ((state >> 24) & 3) as u8
        }).collect();
        for transparent in [None, Some(0)] {
            for max_diff in [0, 64, 2048, 100000] {
                for interlace in [false, true] {
                    let mut image = GiflossyImage::new(&pixels, 3, 4, transparent, Some(&pal));
                    image.interlace = interlace;
                    let root = u16::from(image.px_at_pos(0).unwrap());
                    let mut run = |original| {
                        let mut lookup = Lookup {
                            code_table: &table, pal: &pal, image: &image, max_diff,
                            best_node: root, best_pos: 1, best_total_diff: 0,
                            pending: &mut pending,
                        };
                        let zero = RgbDiff::new(0, 0, 0);
                        if original { recursive(&mut lookup, 1, root, 0, zero); }
                        else { lookup.lossy_node(1, root, 0, zero); }
                        (lookup.best_node, lookup.best_pos, lookup.best_total_diff)
                    };
                    assert_eq!(run(false), run(true), "seed {seed}, transparent {transparent:?}, loss {max_diff}, interlace {interlace}");
                }
            }
        }
    }
}
