//! Fixed shortcuts used before users have had a chance to configure Agere.

use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::key_hint;
use crate::key_hint::KeyBinding;

pub(crate) const MOVE_UP: [KeyBinding; 1] = [key_hint::plain(KeyCode::Up)];
pub(crate) const MOVE_DOWN: [KeyBinding; 1] = [key_hint::plain(KeyCode::Down)];
pub(crate) const SELECT_FIRST: [KeyBinding; 1] = [key_hint::plain(KeyCode::Char('1'))];
pub(crate) const SELECT_SECOND: [KeyBinding; 1] = [key_hint::plain(KeyCode::Char('2'))];
pub(crate) const SELECT_THIRD: [KeyBinding; 1] = [key_hint::plain(KeyCode::Char('3'))];
pub(crate) const CONFIRM: [KeyBinding; 1] = [key_hint::plain(KeyCode::Enter)];
pub(crate) const CANCEL: [KeyBinding; 1] = [key_hint::plain(KeyCode::Esc)];
pub(crate) const QUIT: [KeyBinding; 2] = [
    key_hint::ctrl(KeyCode::Char('c')),
    key_hint::ctrl(KeyCode::Char('d')),
];
pub(crate) const TOGGLE_ANIMATION: [KeyBinding; 2] = [
    key_hint::ctrl(KeyCode::Char('.')),
    KeyBinding::new(
        KeyCode::Char('.'),
        KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
    ),
];
