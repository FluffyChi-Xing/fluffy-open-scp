use sc_argscript::source::{Cause, parse_file, parse_source};
use std::path::Path;

#[test]
fn file_replays_nested_definition_with_physical_origins() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nested.er2");
    let mut seen = vec![];
    parse_file(&path, |site, args| {
        seen.push((
            site.line,
            site.calls.clone(),
            args.iter().map(|a| a.value.to_owned()).collect::<Vec<_>>(),
        ));
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(
        seen,
        [(
            3,
            vec![8, 6],
            vec![
                "emit".to_string(),
                "Garage_A".to_string(),
                "#literal".to_string()
            ]
        )]
    );
}

#[test]
fn replay_strips_injected_comments_and_stops_at_inner_failure() {
    let input = "\u{feff}define Inner(x)\remit &{x} tail\rbad\renddef\r\ndefine Outer()\ncreate Inner(\"# cut\")\nenddef\ncreate Outer()\nemit later";
    let mut seen = vec![];
    let error = parse_source(input, |_, args| {
        if args[0].value == "bad" {
            return Err("unknown command");
        }
        seen.push(args.len());
        Ok(())
    })
    .unwrap_err();
    assert_eq!(seen, [1]);
    assert_eq!(error.site.line, 3);
    assert_eq!(error.site.calls, [8, 6]);
    assert_eq!(error.cause, Cause::Command("unknown command"));
}

#[test]
fn unfinished_input_points_to_the_opening() {
    for (script, line, cause) in [
        ("\n#<\n#<", 3, Cause::UnclosedComment),
        ("define Empty()\n # text", 1, Cause::UnclosedDefinition),
        ("# ignored\n#>", 2, Cause::UnmatchedComment),
    ] {
        let error = parse_source(script, |_, _| Ok::<_, ()>(())).unwrap_err();
        assert_eq!((error.site.line, error.cause), (line, cause));
    }
}

#[test]
fn inactive_or_recursive_input_cannot_emit_spurious_commands() {
    let mut called = false;
    for input in [
        "if false\nemit wrong\nendif",
        "define M()\ncreate M()\nenddef\ncreate M()",
    ] {
        let err = parse_source(input, |_, _| {
            called = true;
            Ok::<_, ()>(())
        })
        .unwrap_err();
        assert!(matches!(
            err.cause,
            Cause::UnsupportedDirective(_) | Cause::ExpansionLimit
        ));
    }
    assert!(!called);
}
