const RESET: &str = "\x1b[0m";
const C1: (u8, u8, u8) = (142, 197, 255);
const C2: (u8, u8, u8) = (43, 127, 255);
const C3: (u8, u8, u8) = (21, 93, 252);
const C4: (u8, u8, u8) = (20, 71, 230);
const C5: (u8, u8, u8) = (25, 60, 184);

pub fn print_banner() {
    print_ansi_banner();
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
