use crate::control::Result;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use zeroize::Zeroizing;

// Semantic roles for the trusted UI; guest colors are rendered separately.
pub const BRAND: &str = "\x1b[95m";
pub const ACTION: &str = "\x1b[96m";
pub const FEEDBACK: &str = "\x1b[93m";
pub const TEXT: &str = "\x1b[37m";
pub const RESET: &str = "\x1b[0m";
pub const CLEAR: &str = "\x1b[?25l\x1b[?7l\x1b[0m\x1b[2J\x1b[H";
pub const LOGO: &str = "   ___  ___ _    _____   _____ ___  _  _\r\n  / _ \\| _ ) |  |_ _\\ \\ / /_ _/ _ \\| \\| |\r\n | (_) | _ \\ |__ | | \\ V / | | (_) | .` |\r\n  \\___/|___/____|___| \\_/ |___\\___/|_|\\_|";

pub struct Terminal {
    file: File,
    saved: libc::termios,
}

impl Terminal {
    pub fn open() -> Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let mut saved = std::mem::MaybeUninit::uninit();
        unsafe {
            if libc::tcgetattr(file.as_raw_fd(), saved.as_mut_ptr()) != 0 {
                return Err("terminal_required".into());
            }
            let saved = saved.assume_init();
            let mut raw = saved;
            libc::cfmakeraw(&mut raw);
            if libc::tcsetattr(file.as_raw_fd(), libc::TCSAFLUSH, &raw) != 0 {
                return Err("terminal_setup_failed".into());
            }
            let mut terminal = Self { file, saved };
            terminal.write("\x1b[?1049h\x1b[?2004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l")?;
            Ok(terminal)
        }
    }

    pub fn size(&self) -> (u16, u16) {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(self.file.as_raw_fd(), libc::TIOCGWINSZ, &mut size) } != 0
            || size.ws_row == 0
            || size.ws_col == 0
        {
            return (24, 80);
        }
        (size.ws_row.clamp(8, 100), size.ws_col.clamp(24, 256))
    }

    pub fn write(&mut self, text: &str) -> Result<()> {
        self.file.write_all(text.as_bytes())?;
        Ok(())
    }

    pub fn flush_input(&self) {
        unsafe {
            libc::tcflush(self.file.as_raw_fd(), libc::TCIFLUSH);
        }
    }

    pub fn input(&mut self, timeout: i32) -> Result<Zeroizing<Vec<u8>>> {
        let mut poll = libc::pollfd {
            fd: self.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, timeout) } < 0 {
            return Err("terminal_poll_failed".into());
        }
        if poll.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Err("terminal_closed".into());
        }
        let mut bytes = Zeroizing::new(vec![0; 4096]);
        if poll.revents & libc::POLLIN == 0 {
            bytes.clear();
            return Ok(bytes);
        }
        let size = self.file.read(&mut bytes)?;
        if size == 0 {
            return Err("terminal_closed".into());
        }
        bytes.truncate(size);
        Ok(bytes)
    }

    pub fn password(&mut self, title: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
        self.flush_input();
        self.write(&format!("{CLEAR}{BRAND} OBLIVION - controllo fidato{RESET}\r\n\r\n {title}\r\n\r\n {ACTION}Invio conferma | Esc annulla{RESET}\r\n\r\n Password nascosta: \x1b[?25h"))?;
        let mut secret = Zeroizing::new(Vec::with_capacity(1024));
        loop {
            let input = self.input(-1)?;
            for byte in input.iter().copied() {
                match byte {
                    27 => {
                        self.flush_input();
                        return Ok(None);
                    }
                    b'\r' | b'\n' => {
                        self.flush_input();
                        return Ok(Some(secret));
                    }
                    8 | 127 => {
                        while let Some(byte) = secret.last_mut() {
                            let continuation = *byte & 0xc0 == 0x80;
                            *byte = 0;
                            secret.pop();
                            if !continuation {
                                break;
                            }
                        }
                    }
                    21 => {
                        secret.fill(0);
                        secret.clear();
                    }
                    32..=126 | 128..=255 => {
                        if secret.len() == 1024 {
                            return Err("password_too_long".into());
                        }
                        secret.push(byte);
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self
            .file
            .write_all(b"\x1b[0m\x1b[?7h\x1b[?25h\x1b[2J\x1b[3J\x1b[?1049l\x1b[2J\x1b[3J\x1b[H");
        unsafe {
            libc::tcsetattr(self.file.as_raw_fd(), libc::TCSAFLUSH, &self.saved);
        }
    }
}

#[derive(Default)]
pub struct Replies {
    bytes: Vec<u8>,
}

impl vt100::Callbacks for Replies {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        first: Option<u8>,
        second: Option<u8>,
        params: &[&[u16]],
        command: char,
    ) {
        if first.is_some() || second.is_some() || self.bytes.len() > 4096 {
            return;
        }
        let value = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        let reply = match (command, value) {
            ('n', 5) => "\x1b[0n".to_owned(),
            ('n', 6) => {
                let (row, col) = screen.cursor_position();
                format!("\x1b[{};{}R", row + 1, col + 1)
            }
            ('t', 18) => {
                let (rows, cols) = screen.size();
                format!("\x1b[8;{rows};{cols}t")
            }
            ('c', 0) => "\x1b[?1;2c".to_owned(),
            _ => String::new(),
        };
        self.bytes.extend_from_slice(reply.as_bytes());
    }
}

pub fn parser(rows: u16, cols: u16) -> vt100::Parser<Replies> {
    vt100::Parser::new_with_callbacks(
        NonZeroU16::new(rows.max(1)).unwrap(),
        NonZeroU16::new(cols.max(1)).unwrap(),
        0,
        Replies::default(),
    )
}

// Serialize visible cells only. OSC, DCS, terminal replies, screen switches,
// window resize and clipboard requests never reach the host terminal.
pub fn render_guest(screen: &vt100::Screen) -> String {
    use std::fmt::Write;
    let mut out = String::from(CLEAR);
    out.push_str(&format!("{BRAND} OBLIVION | SESSIONE APERTA{RESET}\x1b[2;1H{ACTION} Ctrl+] chiude e torna al manager{RESET}"));
    let (rows, cols) = screen.size();
    for row in 0..rows.get() {
        let _ = write!(out, "\x1b[{};1H", row + 4);
        for col in 0..cols.get() {
            let cell = screen.cell(row, col).unwrap();
            if cell.is_wide_continuation() {
                continue;
            }
            out.push_str(RESET);
            color(&mut out, cell.fgcolor(), false);
            color(&mut out, cell.bgcolor(), true);
            if cell.bold() {
                out.push_str("\x1b[1m");
            }
            if cell.underline() {
                out.push_str("\x1b[4m");
            }
            if cell.inverse() {
                out.push_str("\x1b[7m");
            }
            if cell.contents().is_empty() {
                out.push(' ');
            }
            for character in cell.contents().chars().filter(|c| !c.is_control()) {
                out.push(character);
            }
        }
    }
    let (row, col) = screen.cursor_position();
    let _ = write!(
        out,
        "{RESET}\x1b[{};{}H{}",
        row + 4,
        col.min(cols.get() - 1) + 1,
        if screen.hide_cursor() {
            "\x1b[?25l"
        } else {
            "\x1b[?25h"
        }
    );
    out
}

fn color(out: &mut String, value: vt100::Color, background: bool) {
    use std::fmt::Write;
    // Fixed ANSI colors also work on the Linux console. RGB is mapped to the
    // nearest ANSI palette entry, not forwarded as a terminal capability query.
    let palette: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (170, 0, 0),
        (0, 170, 0),
        (170, 85, 0),
        (0, 0, 170),
        (170, 0, 170),
        (0, 170, 170),
        (170, 170, 170),
        (85, 85, 85),
        (255, 85, 85),
        (85, 255, 85),
        (255, 255, 85),
        (85, 85, 255),
        (255, 85, 255),
        (85, 255, 255),
        (255, 255, 255),
    ];
    let rgb = match value {
        vt100::Color::Default => return,
        vt100::Color::Idx(n) if n < 16 => {
            let _ = write!(
                out,
                "\x1b[{}m",
                if background {
                    if n < 8 { 40 + n } else { 100 + n - 8 }
                } else if n < 8 {
                    30 + n
                } else {
                    90 + n - 8
                }
            );
            return;
        }
        vt100::Color::Idx(n) if n >= 232 => {
            let v = 8 + (n - 232) * 10;
            (v, v, v)
        }
        vt100::Color::Idx(n) => {
            let n = n - 16;
            let channel = [0, 95, 135, 175, 215, 255];
            (
                channel[(n / 36) as usize],
                channel[((n / 6) % 6) as usize],
                channel[(n % 6) as usize],
            )
        }
        vt100::Color::Rgb(r, g, b) => (r, g, b),
    };
    let index = palette
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| {
            (c.0 as i32 - rgb.0 as i32).pow(2)
                + (c.1 as i32 - rgb.1 as i32).pow(2)
                + (c.2 as i32 - rgb.2 as i32).pow(2)
        })
        .unwrap()
        .0 as u8;
    color(out, vt100::Color::Idx(index), background);
}

pub fn bridge(terminal: &mut Terminal, mut stream: UnixStream) -> Result<()> {
    stream.set_nonblocking(true)?;
    let (rows, cols) = terminal.size();
    let mut parser = parser(rows - 3, cols);
    terminal.flush_input();
    terminal.write(&render_guest(parser.screen()))?;
    let mut bytes = [0u8; 4096];
    loop {
        let input = terminal.input(20)?;
        if input.contains(&29) {
            terminal.flush_input();
            return Ok(());
        }
        if !input.is_empty() {
            stream.write_all(&input)?;
        }
        match stream.read(&mut bytes) {
            Ok(0) => return Err("console_closed".into()),
            Ok(size) => {
                parser.process(&bytes[..size]);
                if !parser.callbacks().bytes.is_empty() {
                    stream.write_all(&parser.callbacks().bytes)?;
                    parser.callbacks_mut().bytes.clear();
                }
                terminal.write(&render_guest(parser.screen()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
}
