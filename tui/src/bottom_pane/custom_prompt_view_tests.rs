use super::*;
use pretty_assertions::assert_eq;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use std::time::Instant;

#[test]
fn paste_burst_newline_does_not_submit_short_first_line() {
    let now = Instant::now();

    for (first_line, second_line) in [("id1", "body"), ("foo", "bar")] {
        let (mut view, submitted_rx) = custom_prompt_view();
        let mut ms = 0;

        for ch in first_line.chars() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
            ms += 1;
        }
        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(ms));
        ms += 1;
        for ch in second_line.chars() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
            ms += 1;
        }

        assert!(submitted_rx.try_recv().is_err());
        assert!(!view.is_complete());

        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(200));

        assert_eq!(
            submitted_rx.try_recv(),
            Ok(format!("{first_line}\n{second_line}"))
        );
        assert!(view.is_complete());
    }
}

#[test]
fn paste_burst_newline_after_tab_does_not_submit() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();
    let mut ms = 0;

    view.handle_key_event_at(KeyEvent::from(KeyCode::Char('x')), now + elapsed(ms));
    ms += 1;
    view.handle_key_event_at(KeyEvent::from(KeyCode::Tab), now + elapsed(ms));
    ms += 1;
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(ms));
    ms += 1;
    for ch in "rest".chars() {
        view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
        ms += 1;
    }

    assert!(submitted_rx.try_recv().is_err());
    assert!(!view.is_complete());

    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(200));

    assert_eq!(submitted_rx.try_recv(), Ok("x\nrest".to_string()));
    assert!(view.is_complete());
}

#[test]
fn delayed_enter_after_typing_submits() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();

    for (idx, ch) in "foo".chars().enumerate() {
        view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(idx * 80));
    }
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(260));

    assert_eq!(submitted_rx.try_recv(), Ok("foo".to_string()));
    assert!(view.is_complete());
}

#[test]
fn fast_enter_after_single_typed_char_submits() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();

    view.handle_key_event_at(KeyEvent::from(KeyCode::Char('y')), now);
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(1));

    assert_eq!(submitted_rx.try_recv(), Ok("y".to_string()));
    assert!(view.is_complete());
}

#[test]
fn shifted_enter_inserts_newline_without_submitting() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();

    view.handle_key_event_at(KeyEvent::from(KeyCode::Char('a')), now);
    view.handle_key_event_at(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        now + elapsed(1),
    );
    view.handle_key_event_at(KeyEvent::from(KeyCode::Char('b')), now + elapsed(2));

    assert!(submitted_rx.try_recv().is_err());
    assert!(!view.is_complete());

    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(200));

    assert_eq!(submitted_rx.try_recv(), Ok("a\nb".to_string()));
    assert!(view.is_complete());
}

fn custom_prompt_view() -> (CustomPromptView, Receiver<String>) {
    let (submitted, submitted_rx) = std::sync::mpsc::channel();
    let view = CustomPromptView::new(
        "Edit goal".to_string(),
        "Type a goal objective and press Enter".to_string(),
        String::new(),
        None,
        Box::new(move |text| {
            submitted.send(text).expect("send submitted text");
        }),
    );
    (view, submitted_rx)
}

fn elapsed(ms: usize) -> Duration {
    Duration::from_millis(ms as u64)
}
