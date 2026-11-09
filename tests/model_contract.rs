use std::time::{Duration, UNIX_EPOCH};

use sagebrush::model::{
    CurrentWindow, Field, InvokingContext, Session, SessionId, SessionSnapshot, WindowId,
    decode_name, escape_text, format_uptime, positive_field, uptime,
};

fn session(id: &str) -> Session {
    Session {
        id: SessionId::parse(id).unwrap(),
        name: Field::Known("name".to_owned()),
        window_count: Field::Known(1),
        created_at: Field::Known(UNIX_EPOCH),
        current_window: Field::Unavailable("not queried".into()),
        active: Field::Known(false),
    }
}

#[test]
fn session_ids_are_strict_unique_and_sorted_numerically() {
    assert!(SessionId::parse("1").is_err());
    assert!(SessionId::parse("$").is_err());
    assert!(SessionId::parse("$-1").is_err());
    assert!(SessionId::parse("$1x").is_err());
    let snapshot = SessionSnapshot::new(
        vec![session("$10"), session("$2"), session("$1")],
        UNIX_EPOCH,
        InvokingContext::OutsideTmux,
    )
    .unwrap();
    let ids: Vec<_> = snapshot.sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["$1", "$2", "$10"]);
    assert!(
        SessionSnapshot::new(
            vec![session("$1"), session("$1")],
            UNIX_EPOCH,
            InvokingContext::OutsideTmux
        )
        .is_err()
    );
    assert!(
        SessionSnapshot::new(
            vec![session("$1"), session("$01")],
            UNIX_EPOCH,
            InvokingContext::OutsideTmux
        )
        .is_err()
    );
}

#[test]
fn missing_invalid_and_independently_invalid_dimensions_are_unavailable() {
    assert_eq!(positive_field("80", "width"), Field::Known(80));
    assert!(matches!(
        positive_field("0", "width"),
        Field::Unavailable(_)
    ));
    assert!(matches!(
        positive_field("-1", "height"),
        Field::Unavailable(_)
    ));
    assert!(matches!(positive_field("", "width"), Field::Unavailable(_)));
    assert!(matches!(
        positive_field("4294967296", "height"),
        Field::Unavailable(_)
    ));
    assert!(matches!(
        positive_field("+1", "height"),
        Field::Unavailable(_)
    ));
    assert_eq!(positive_field("24", "height"), Field::Known(24));
}

#[test]
fn snapshot_marks_each_invalid_known_field_unavailable() {
    let mut row = session("$9");
    row.window_count = Field::Known(0);
    row.current_window = Field::Known(CurrentWindow {
        id: WindowId::parse("@1").unwrap(),
        width: Field::Known(0),
        height: Field::Known(24),
    });
    let snapshot =
        SessionSnapshot::new(vec![row], UNIX_EPOCH, InvokingContext::OutsideTmux).unwrap();
    assert!(matches!(
        snapshot.sessions[0].window_count,
        Field::Unavailable(_)
    ));
    let Field::Known(window) = &snapshot.sessions[0].current_window else {
        panic!("current window must remain available");
    };
    assert!(matches!(window.width, Field::Unavailable(_)));
    assert_eq!(window.height, Field::Known(24));
    assert_eq!(snapshot.issues.len(), 2);
}

#[test]
fn names_are_preserved_or_explicitly_unavailable_and_controls_are_escaped() {
    assert_eq!(
        decode_name(b" space\tline\n\\\x1b ", "name"),
        Field::Known(" space\tline\n\\\x1b ".to_owned())
    );
    assert!(matches!(
        decode_name(&[0xff], "name"),
        Field::Unavailable(_)
    ));
    assert_eq!(
        escape_text(" space\tline\n\\\x1b "),
        " space\\tline\\n\\\\\\u{1b} "
    );
}

#[test]
fn uptime_uses_whole_seconds_without_clamping_future_creation_times() {
    let captured = UNIX_EPOCH + Duration::from_secs(3);
    assert_eq!(
        uptime(
            &Field::Known(UNIX_EPOCH + Duration::from_millis(1500)),
            captured
        ),
        Field::Known(1)
    );
    assert!(matches!(
        uptime(&Field::Known(captured + Duration::from_secs(1)), captured),
        Field::Unavailable(_)
    ));
    assert_eq!(format_uptime(0), "0s");
    assert_eq!(format_uptime(86_400), "1d");
    assert_eq!(format_uptime(82_859), "23h0m59s");
    assert_eq!(format_uptime(3_719), "1h1m59s");
}

#[test]
fn active_status_follows_context_and_never_guesses_a_missing_identity() {
    let rows = vec![session("$1"), session("$2")];
    let snapshot = SessionSnapshot::new(
        rows.clone(),
        UNIX_EPOCH,
        InvokingContext::InsideTmux {
            session_id: SessionId::parse("$2").unwrap(),
        },
    )
    .unwrap();
    assert_eq!(snapshot.sessions[0].active, Field::Known(false));
    assert_eq!(snapshot.sessions[1].active, Field::Known(true));

    let snapshot = SessionSnapshot::new(
        rows,
        UNIX_EPOCH,
        InvokingContext::InsideTmux {
            session_id: SessionId::parse("$3").unwrap(),
        },
    )
    .unwrap();
    assert!(matches!(
        snapshot.context,
        InvokingContext::ActiveUnavailable
    ));
    assert!(
        snapshot
            .sessions
            .iter()
            .all(|row| matches!(row.active, Field::Unavailable(_)))
    );
}
