use ui_inspect::{Element, InspectError, Point, Rect, Snapshot, ViewIdentity};

const VIEWPORT: Rect = Rect {
    left: 0.0,
    top: 0.0,
    right: 100.0,
    bottom: 80.0,
};

fn snapshot(page: &str, elements: Vec<Element<u32>>) -> Snapshot<u32> {
    Snapshot::from_preorder(
        ViewIdentity {
            build: "test-build".into(),
            page: page.into(),
        },
        VIEWPORT,
        elements,
    )
    .unwrap()
}

fn element(key: u32, depth: u32, visible: bool) -> Element<u32> {
    Element {
        key: Some(key),
        depth,
        visible,
        bounds: Some(VIEWPORT),
    }
}

#[test]
fn maps_displayed_content_and_rejects_margins_and_edges() {
    let display = Rect {
        left: 15.0,
        top: 20.0,
        right: 215.0,
        bottom: 180.0,
    };
    assert_eq!(
        display.map_point(Point { x: 115.0, y: 100.0 }, VIEWPORT),
        Some(Point { x: 50.0, y: 40.0 })
    );
    assert_eq!(
        display.map_point(Point { x: 14.0, y: 100.0 }, VIEWPORT),
        None
    );
    assert_eq!(
        display.map_point(Point { x: 215.0, y: 100.0 }, VIEWPORT),
        None
    );
}

#[test]
fn offers_visible_hit_candidates_before_their_parents() {
    let scene = snapshot(
        "options",
        vec![
            element(1, 0, true),
            element(2, 1, true),
            element(3, 1, false),
        ],
    );
    assert_eq!(
        scene
            .hit_candidates(Point { x: 10.0, y: 10.0 })
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    assert_eq!(scene.hit_candidates(Point { x: 110.0, y: 10.0 }).count(), 0);
}

#[test]
fn resolves_after_reordering_and_reports_ambiguous_or_stale_paths() {
    let root = element(1, 0, true);
    let target = element(2, 1, true);
    let sibling = element(3, 1, true);
    let scene = snapshot(
        "options",
        vec![root.clone(), target.clone(), sibling.clone()],
    );
    let marker = scene.mark(1).unwrap();
    let reordered = snapshot("options", vec![root.clone(), sibling, target.clone()]);
    assert_eq!(reordered.resolve(&marker), Ok(2));
    let missing = snapshot("options", vec![root.clone()]);
    assert_eq!(missing.resolve(&marker), Err(InspectError::Missing));
    let duplicate = snapshot(
        "options",
        vec![root.clone(), target.clone(), target.clone()],
    );
    assert_eq!(duplicate.resolve(&marker), Err(InspectError::Ambiguous));
    let other_page = snapshot("loading", vec![root, target]);
    assert_eq!(
        other_page.resolve(&marker),
        Err(InspectError::DifferentView)
    );
}

#[test]
fn refuses_a_child_without_its_parent() {
    let result = Snapshot::from_preorder(
        ViewIdentity {
            build: "test-build".into(),
            page: "options".into(),
        },
        VIEWPORT,
        vec![element(2, 1, true)],
    );
    assert!(matches!(result, Err(InspectError::InvalidDepth(0))));
}
