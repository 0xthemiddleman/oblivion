use oblivion_host::terminal::{parser, render_guest};

#[test]
fn guest_controls_cannot_escape_the_screen_or_change_trusted_header() {
    for (rows, cols) in [(24, 80), (10, 32)] {
        let mut guest = parser(rows - 3, cols);
        guest.process(b"\x1b]52;c;U0VDUkVU\x07\x1b]2;HOSTILE_TITLE\x07\x1b[8;999;999t\x1b[?1049h\x1b[2J\x1b[1;1HFake manager\x1b[999;999HEDGE\x1bPprivate-dcs\x1b\\");
        guest.process(b"\x1b[2;1HVisible guest text");
        let rendered = render_guest(guest.screen());
        assert!(!rendered.contains("U0VDUkVU"));
        assert!(!rendered.contains("HOSTILE_TITLE"));
        assert!(!rendered.contains("private-dcs"));
        assert!(!rendered.contains("\x1b]"));
        let mut host = parser(rows, cols);
        host.process(rendered.as_bytes());
        assert!(
            host.screen()
                .rows(0, cols)
                .next()
                .unwrap()
                .starts_with(" OBLIVION | SESSIONE APERTA")
        );
        assert!(
            host.screen()
                .rows(0, cols)
                .nth(1)
                .unwrap()
                .starts_with(" Ctrl+]")
        );
        assert!(!host.screen().alternate_screen());
        assert!(
            host.screen()
                .rows(0, cols)
                .nth(4)
                .unwrap()
                .starts_with("Visible guest text")
        );
    }
}

#[test]
fn fragmented_and_long_escape_strings_remain_internal() {
    let mut guest = parser(21, 80);
    for byte in b"\x1b]52;c;secret\x07\x1b[31mvisible" {
        guest.process(&[*byte]);
    }
    guest.process(b"\x1b]");
    for _ in 0..10000 {
        guest.process(&[b'x'; 128]);
    }
    guest.process(b"\x07\x1b[2J\x1b[Hafter");
    let rendered = render_guest(guest.screen());
    assert!(rendered.len() < 100000);
    assert!(!rendered.contains("secret"));
    let mut host = parser(24, 80);
    host.process(rendered.as_bytes());
    assert!(host.screen().contents().contains("after"));
    assert!(host.screen().contents().starts_with(" OBLIVION"));
}
