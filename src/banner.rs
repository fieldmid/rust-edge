use std::{
    io::{self, IsTerminal, Write},
    thread,
    time::Duration,
};

const RESET: &str = "\x1b[0m";
const C1: (u8, u8, u8) = (142, 197, 255);
const C2: (u8, u8, u8) = (43, 127, 255);
const C3: (u8, u8, u8) = (21, 93, 252);
const C4: (u8, u8, u8) = (20, 71, 230);
const C5: (u8, u8, u8) = (25, 60, 184);
const WORDMARK_COLORS: &[(u8, u8, u8)] = &[C1, C2, C3, C4, C5, C4, C3, C2];

pub fn print_banner() {
    print_wordmark_animation();
    print_ansi_banner();
}

fn print_wordmark_animation() {
    if !io::stdout().is_terminal() {
        println!("fieldmid");
        return;
    }

    let mut stdout = io::stdout();
    let letters = ['f', 'i', 'e', 'l', 'd', 'm', 'i', 'd'];

    let _ = write!(stdout, "\n  ");
    for (index, letter) in letters.iter().enumerate() {
        let color = WORDMARK_COLORS[index % WORDMARK_COLORS.len()];
        let _ = write!(stdout, "{}", paint(color, &letter.to_string()));
        if index < letters.len() - 1 {
            let _ = write!(stdout, " ");
        }
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(95));
    }
    let _ = writeln!(stdout, "\n");
}

fn print_ansi_banner() {
    println!(
        "{}{}{}{}{}{}{}{}",
        paint(C5, "███████╗"),
        paint(C4, "██╗"),
        paint(C3, "███████╗"),
        paint(C4, "██╗     "),
        paint(C3, "██████╗ "),
        paint(C2, "███╗   ███╗"),
        paint(C1, "██╗"),
        paint(C3, "██████╗ ")
    );
    println!(
        "{}{}{}{}{}{}{}{}",
        paint(C5, "██╔════╝"),
        paint(C4, "██║"),
        paint(C3, "██╔════╝"),
        paint(C4, "██║     "),
        paint(C3, "██╔══██╗"),
        paint(C2, "████╗ ████║"),
        paint(C1, "██║"),
        paint(C3, "██╔══██╗")
    );
    println!(
        "{}{}{}{}{}{}{}{}",
        paint(C4, "█████╗  "),
        paint(C3, "██║"),
        paint(C2, "█████╗  "),
        paint(C3, "██║     "),
        paint(C4, "██║  ██║"),
        paint(C2, "██╔████╔██║"),
        paint(C1, "██║"),
        paint(C4, "██║  ██║")
    );
    println!(
        "{}{}{}{}{}{}{}{}",
        paint(C3, "██╔══╝  "),
        paint(C4, "██║"),
        paint(C4, "██╔══╝  "),
        paint(C2, "██║     "),
        paint(C3, "██║  ██║"),
        paint(C4, "██║╚██╔╝██║"),
        paint(C2, "██║"),
        paint(C3, "██║  ██║")
    );
    println!(
        "{}{}{}{}{}{}{}{}",
        paint(C2, "██║     "),
        paint(C3, "██║"),
        paint(C4, "███████╗"),
        paint(C3, "███████╗"),
        paint(C2, "██████╔╝"),
        paint(C3, "██║ ╚═╝ ██║"),
        paint(C4, "██║"),
        paint(C5, "██████╔╝")
    );
    println!(
        "{}{}{}{}{}{}{}{}",
        paint(C1, "╚═╝     "),
        paint(C2, "╚═╝"),
        paint(C3, "╚══════╝"),
        paint(C4, "╚══════╝"),
        paint(C3, "╚═════╝ "),
        paint(C5, "╚═╝     ╚═╝"),
        paint(C4, "╚═╝"),
        paint(C5, "╚═════╝ ")
    );
}

fn paint((r, g, b): (u8, u8, u8), text: &str) -> String {
    format!("\x1b[38;2;{r};{g};{b}m{text}{RESET}")
}
