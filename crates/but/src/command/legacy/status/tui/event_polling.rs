#[cfg(not(unix))]
use std::io::Read;
use std::{
    collections::VecDeque,
    convert::Infallible,
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};

use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    terminal,
};

use termwiz::input::{InputEvent, InputParser, KeyCode as TermwizKeyCode, Modifiers};

use crate::tui::keyboard::{self, KeyboardMode};

/// Trait for abstracting event polling so we can hardcode events in tests.
pub(super) trait EventPolling {
    type Error: std::error::Error + Send + Sync + 'static;

    fn poll(&mut self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error>;
}

/// An [`EventPolling`] implementation that polls terminal input from a single raw stdin reader.
///
/// We intentionally don't use `crossterm::event::read()` here. Some terminals only report
/// Shift+Enter reliably after Pi-style keyboard negotiation, and mixing Crossterm's event reader
/// with our own raw reader would race on stdin.
pub(super) struct RawEventPolling {
    stdin_rx: Receiver<u8>,
    pending_input: VecDeque<Vec<u8>>,
    previous_size: Option<(u16, u16)>,
    input_parser: InputParser,
    original_panic_hook: Arc<Mutex<Option<PanicHook>>>,
}

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

impl RawEventPolling {
    const PACKET_FOLLOWUP_TIMEOUT: Duration = Duration::from_millis(10);
    const KEYBOARD_QUERY_TIMEOUT: Duration = Duration::from_millis(150);

    pub(super) fn new() -> io::Result<Self> {
        let stdin_rx = spawn_stdin_reader();
        let mut pending_input = VecDeque::new();
        let keyboard_mode = setup_keyboard(&stdin_rx, &mut pending_input)?;
        keyboard::set_active_mode(keyboard_mode);

        let original_panic_hook: Arc<Mutex<Option<PanicHook>>> =
            Arc::new(Mutex::new(Some(std::panic::take_hook())));
        let hook_ref = Arc::clone(&original_panic_hook);
        std::panic::set_hook(Box::new(move |panic_info| {
            keyboard::restore_active_mode();
            if let Some(hook) = hook_ref.lock().ok().and_then(|mut hook| hook.take()) {
                hook(panic_info);
            }
        }));

        Ok(Self {
            stdin_rx,
            pending_input,
            previous_size: terminal::size().ok(),
            input_parser: InputParser::new(),
            original_panic_hook,
        })
    }
}

impl Drop for RawEventPolling {
    fn drop(&mut self) {
        keyboard::restore_active_mode();
        if let Some(hook) = self
            .original_panic_hook
            .lock()
            .ok()
            .and_then(|mut hook| hook.take())
        {
            std::panic::set_hook(hook);
        }
    }
}

impl EventPolling for RawEventPolling {
    type Error = io::Error;

    fn poll(&mut self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        let mut events = Vec::new();

        if let Some(bytes) = self.pending_input.pop_front() {
            events.extend(parse_input_bytes(&mut self.input_parser, &bytes));
            drain_available_packets(&self.stdin_rx, &mut self.input_parser, &mut events);
        } else if let Some(bytes) = read_input_packet_timeout(&self.stdin_rx, timeout)? {
            events.extend(parse_input_bytes(&mut self.input_parser, &bytes));
            drain_available_packets(&self.stdin_rx, &mut self.input_parser, &mut events);
        }

        if let Ok(size) = terminal::size()
            && self
                .previous_size
                .is_some_and(|previous_size| previous_size != size)
        {
            events.push(Event::Resize(size.0, size.1));
            self.previous_size = Some(size);
        }

        Ok(events)
    }
}

/// An [`EventPolling`] implementation that never yields events.
///
/// This is used for non-interactive runs where touching terminal input can stop the process when
/// profilers launch the target in a background process group.
#[derive(Copy, Clone)]
pub(super) struct NoopEventPolling;

impl EventPolling for NoopEventPolling {
    type Error = Infallible;

    fn poll(&mut self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(None)
    }
}

fn setup_keyboard(
    stdin_rx: &Receiver<u8>,
    pending_input: &mut VecDeque<Vec<u8>>,
) -> io::Result<KeyboardMode> {
    keyboard::query_kitty_keyboard_protocol()?;

    match read_input_packet_timeout(stdin_rx, RawEventPolling::KEYBOARD_QUERY_TIMEOUT)? {
        Some(bytes) if is_kitty_protocol_response(&bytes) => {
            keyboard::enable_mode(KeyboardMode::Kitty)?;
            Ok(KeyboardMode::Kitty)
        }
        Some(bytes) => {
            pending_input.push_back(bytes);
            keyboard::enable_mode(KeyboardMode::ModifyOtherKeys)?;
            Ok(KeyboardMode::ModifyOtherKeys)
        }
        None => {
            keyboard::enable_mode(KeyboardMode::ModifyOtherKeys)?;
            Ok(KeyboardMode::ModifyOtherKeys)
        }
    }
}

fn is_kitty_protocol_response(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };

    let Some(body) = text
        .strip_prefix("\x1b[?")
        .and_then(|text| text.strip_suffix('u'))
    else {
        return false;
    };

    !body.is_empty() && body.bytes().all(|byte| byte.is_ascii_digit())
}

fn spawn_stdin_reader() -> Receiver<u8> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        loop {
            if keyboard::terminal_input_suspended() {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            match read_stdin_byte_timeout(Duration::from_millis(10)) {
                Ok(Some(byte)) => {
                    if tx.send(byte).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(_) => break,
            }
        }
    });

    rx
}

#[cfg(unix)]
fn read_stdin_byte_timeout(timeout: Duration) -> io::Result<Option<u8>> {
    let stdin = io::stdin();
    let mut poll_fds = [rustix::event::PollFd::new(
        &stdin,
        rustix::event::PollFlags::IN,
    )];
    let timeout = rustix::event::Timespec {
        tv_sec: timeout
            .as_secs()
            .min(i64::MAX as u64)
            .try_into()
            .unwrap_or(i64::MAX),
        tv_nsec: timeout.subsec_nanos().into(),
    };

    let ready = rustix::event::poll(&mut poll_fds, Some(&timeout))
        .map_err(|err| io::Error::from_raw_os_error(err.raw_os_error()))?;
    if ready == 0 || keyboard::terminal_input_suspended() {
        return Ok(None);
    }

    let mut byte = [0u8];
    let bytes_read = rustix::io::read(&stdin, &mut byte)
        .map_err(|err| io::Error::from_raw_os_error(err.raw_os_error()))?;
    if bytes_read == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "stdin closed"));
    }

    Ok(Some(byte[0]))
}

#[cfg(not(unix))]
fn read_stdin_byte_timeout(timeout: Duration) -> io::Result<Option<u8>> {
    if keyboard::terminal_input_suspended() {
        thread::sleep(timeout);
        return Ok(None);
    }

    let mut byte = [0u8];
    match io::stdin().lock().read_exact(&mut byte) {
        Ok(()) => Ok(Some(byte[0])),
        Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(err) => Err(err),
    }
}

fn read_input_packet_timeout(
    stdin_rx: &Receiver<u8>,
    timeout: Duration,
) -> io::Result<Option<Vec<u8>>> {
    match stdin_rx.recv_timeout(timeout) {
        Ok(first) => Ok(Some(read_packet_after_first(first, stdin_rx))),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    }
}

fn read_packet_after_first(first: u8, stdin_rx: &Receiver<u8>) -> Vec<u8> {
    let mut bytes = vec![first];
    while let Ok(byte) = stdin_rx.recv_timeout(RawEventPolling::PACKET_FOLLOWUP_TIMEOUT) {
        bytes.push(byte);
    }

    bytes
}

fn drain_available_packets(
    stdin_rx: &Receiver<u8>,
    input_parser: &mut InputParser,
    events: &mut Vec<Event>,
) {
    while let Ok(first) = stdin_rx.try_recv() {
        let bytes = read_packet_after_first(first, stdin_rx);
        events.extend(parse_input_bytes(input_parser, &bytes));
    }
}

fn parse_input_bytes(input_parser: &mut InputParser, raw: &[u8]) -> Vec<Event> {
    let normalized = normalize_kitty_keyboard_events(raw);
    input_parser
        .parse_as_vec(&normalized, false)
        .into_iter()
        .filter_map(termwiz_event_to_crossterm)
        .collect()
}

fn normalize_kitty_keyboard_events(raw: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(raw.len());
    let mut index = 0;

    while index < raw.len() {
        if raw[index..].starts_with(b"\x1b[")
            && let Some(sequence_end) = raw[index + 2..]
                .iter()
                .position(|byte| (0x40..=0x7e).contains(byte))
                .map(|offset| index + 2 + offset)
            && raw[sequence_end] == b'u'
        {
            let body = &raw[index + 2..sequence_end];
            if let Some(body) = normalize_kitty_csi_u_body(body) {
                normalized.extend_from_slice(b"\x1b[");
                normalized.extend_from_slice(&body);
                normalized.push(b'u');
            }
            index = sequence_end + 1;
        } else {
            normalized.push(raw[index]);
            index += 1;
        }
    }

    normalized
}

fn normalize_kitty_csi_u_body(body: &[u8]) -> Option<Vec<u8>> {
    if matches!(body.first(), Some(b'?' | b'>' | b'<')) {
        return None;
    }
    if !body.first().is_some_and(u8::is_ascii_digit) {
        return Some(body.to_vec());
    }

    let mut parts = body.split(|byte| *byte == b';');
    let codepoint = parts.next()?;
    let modifiers = parts.next();

    let codepoint = codepoint.split(|byte| *byte == b':').next()?;
    let Some(modifiers) = modifiers else {
        return Some(codepoint.to_vec());
    };

    let mut modifier_parts = modifiers.split(|byte| *byte == b':');
    let modifiers = modifier_parts.next()?;
    let event_type = modifier_parts.next();
    if event_type == Some(b"3".as_slice()) {
        return None;
    }

    let mut normalized = Vec::with_capacity(codepoint.len() + modifiers.len() + 1);
    normalized.extend_from_slice(codepoint);
    normalized.push(b';');
    normalized.extend_from_slice(modifiers);
    Some(normalized)
}

fn termwiz_event_to_crossterm(event: InputEvent) -> Option<Event> {
    match event {
        InputEvent::Key(key_event) => {
            let mut modifiers = termwiz_modifiers_to_crossterm(key_event.modifiers);
            let mut code = termwiz_key_code_to_crossterm(key_event.key)?;
            if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
                code = KeyCode::BackTab;
            }
            if let KeyCode::Char(ch) = code
                && ch.is_ascii_uppercase()
            {
                modifiers |= KeyModifiers::SHIFT;
            }
            Some(key(code, modifiers))
        }
        InputEvent::Paste(text) => Some(Event::Paste(text)),
        InputEvent::Resized { cols, rows } => Some(Event::Resize(cols as u16, rows as u16)),
        InputEvent::Mouse(_) | InputEvent::PixelMouse(_) | InputEvent::Wake => None,
    }
}

fn termwiz_key_code_to_crossterm(code: TermwizKeyCode) -> Option<KeyCode> {
    match code {
        TermwizKeyCode::Char('\r' | '\n') => Some(KeyCode::Enter),
        TermwizKeyCode::Char('\t') => Some(KeyCode::Tab),
        TermwizKeyCode::Char('\u{1b}') => Some(KeyCode::Esc),
        TermwizKeyCode::Char('\u{8}' | '\u{7f}') => Some(KeyCode::Backspace),
        TermwizKeyCode::Char(ch) if ch.is_control() => None,
        TermwizKeyCode::Char(ch) => Some(KeyCode::Char(ch)),
        TermwizKeyCode::Backspace => Some(KeyCode::Backspace),
        TermwizKeyCode::Tab => Some(KeyCode::Tab),
        TermwizKeyCode::Enter => Some(KeyCode::Enter),
        TermwizKeyCode::Escape => Some(KeyCode::Esc),
        TermwizKeyCode::PageUp => Some(KeyCode::PageUp),
        TermwizKeyCode::PageDown => Some(KeyCode::PageDown),
        TermwizKeyCode::End => Some(KeyCode::End),
        TermwizKeyCode::Home => Some(KeyCode::Home),
        TermwizKeyCode::LeftArrow | TermwizKeyCode::ApplicationLeftArrow => Some(KeyCode::Left),
        TermwizKeyCode::RightArrow | TermwizKeyCode::ApplicationRightArrow => Some(KeyCode::Right),
        TermwizKeyCode::UpArrow | TermwizKeyCode::ApplicationUpArrow => Some(KeyCode::Up),
        TermwizKeyCode::DownArrow | TermwizKeyCode::ApplicationDownArrow => Some(KeyCode::Down),
        TermwizKeyCode::Insert => Some(KeyCode::Insert),
        TermwizKeyCode::Delete => Some(KeyCode::Delete),
        TermwizKeyCode::Function(number) => Some(KeyCode::F(number)),
        _ => None,
    }
}

fn termwiz_modifiers_to_crossterm(modifiers: Modifiers) -> KeyModifiers {
    let mut result = KeyModifiers::NONE;
    if modifiers.contains(Modifiers::SHIFT) {
        result |= KeyModifiers::SHIFT;
    }
    if modifiers.contains(Modifiers::ALT) {
        result |= KeyModifiers::ALT;
    }
    if modifiers.contains(Modifiers::CTRL) {
        result |= KeyModifiers::CONTROL;
    }
    if modifiers.contains(Modifiers::SUPER) {
        result |= KeyModifiers::SUPER;
    }
    result
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_events(raw: &[u8]) -> Vec<Event> {
        parse_input_bytes(&mut InputParser::new(), raw)
    }

    fn single_key(raw: &[u8]) -> (KeyCode, KeyModifiers) {
        let events = parse_events(raw);
        assert_eq!(events.len(), 1, "packet should parse as one event");
        let Event::Key(key) = events[0] else {
            panic!("packet should parse as key event");
        };
        (key.code, key.modifiers)
    }

    #[test]
    fn parses_enter_forms() {
        assert_eq!(single_key(b"\r"), (KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            single_key(b"\x1b[27;1:1u"),
            (KeyCode::Esc, KeyModifiers::NONE)
        );
        assert_eq!(
            single_key(b"\x1b[13;2u"),
            (KeyCode::Enter, KeyModifiers::SHIFT)
        );
        assert_eq!(
            single_key(b"\x1b[27;2;13~"),
            (KeyCode::Enter, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn parses_existing_navigation_shortcuts() {
        assert_eq!(single_key(b"j"), (KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(single_key(b"k"), (KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(single_key(b"J"), (KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(single_key(b"K"), (KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(single_key(b"\x1b[A"), (KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(single_key(b"\x1b[B"), (KeyCode::Down, KeyModifiers::NONE));
    }

    #[test]
    fn parses_existing_modified_shortcuts() {
        assert_eq!(
            single_key(b"\x12"),
            (KeyCode::Char('r'), KeyModifiers::CONTROL)
        );
        assert_eq!(
            single_key(b"\x1b[101;3u"),
            (KeyCode::Char('e'), KeyModifiers::ALT)
        );
        assert_eq!(
            single_key(b"\x1b[106;1:1u"),
            (KeyCode::Char('j'), KeyModifiers::NONE)
        );
        assert_eq!(
            parse_events(b"\x1b[106;1:3u"),
            Vec::<Event>::new(),
            "Kitty keyboard-protocol release events are ignored"
        );
        assert_eq!(
            parse_events(b"k\x1b[107;1:3u"),
            vec![key(KeyCode::Char('k'), KeyModifiers::NONE)],
            "printable input followed by its Kitty release event parses as one key"
        );
        assert_eq!(
            parse_events(b"\x1b[?31u"),
            Vec::<Event>::new(),
            "late Kitty keyboard protocol reports are ignored"
        );
        assert_eq!(
            parse_events(b"\x1b[?31uj"),
            vec![key(KeyCode::Char('j'), KeyModifiers::NONE)],
            "late Kitty keyboard protocol reports do not leak their final 'u' as input"
        );
        assert_eq!(
            single_key(b"\x1be"),
            (KeyCode::Char('e'), KeyModifiers::ALT)
        );
    }

    #[test]
    fn parses_text_and_paste() {
        let events = parse_events("æB".as_bytes());
        assert_eq!(events.len(), 2, "printable packet should split into chars");
        assert_eq!(
            events[1],
            key(KeyCode::Char('B'), KeyModifiers::SHIFT),
            "uppercase printable input keeps shift for key binds"
        );

        assert_eq!(
            parse_events(b"\x1b[200~hello\nworld\x1b[201~"),
            vec![Event::Paste("hello\nworld".to_owned())]
        );
    }
}
