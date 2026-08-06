use bam_core::highlight::{Decoration, resolve};

fn deco(background: Option<&str>, priority: i32) -> Decoration {
    Decoration {
        gutter: None,
        badge: None,
        background: background.map(String::from),
        priority,
    }
}

#[test]
fn highest_priority_background_wins() {
    let tokens = resolve(&[deco(Some("low"), 5), deco(Some("high"), 10)]);
    assert_eq!(tokens.background.as_deref(), Some("high"));
}

#[test]
fn equal_priority_backgrounds_resolve_to_the_first_one_seen() {
    // Order in the input list, not hash order, decides the tie: the first
    // decoration to reach the max priority is never displaced by a later
    // one at the same priority.
    let tokens = resolve(&[deco(Some("first"), 5), deco(Some("second"), 5)]);
    assert_eq!(tokens.background.as_deref(), Some("first"));
}

#[test]
fn four_stacking_gutters_render_three() {
    let decorations: Vec<Decoration> = (0..4)
        .map(|i| Decoration {
            gutter: Some(format!("g{i}")),
            badge: None,
            background: None,
            priority: i,
        })
        .collect();
    let tokens = resolve(&decorations);
    assert_eq!(tokens.gutters.len(), 3);
    // Highest priority first: g3, g2, g1 — g0 is the one dropped by the cap.
    assert_eq!(tokens.gutters, vec!["g3", "g2", "g1"]);
}
