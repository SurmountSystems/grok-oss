//! No-echo API key prompt. This crate does not depend on `rpassword`.

use std::io::{self, BufRead, Write};

/// No-echo prompt for an API key.
///
/// On Unix, disable terminal echo when stdin is a tty. Otherwise read one line.
pub fn prompt_api_key_no_echo(prompt: &str) -> io::Result<String> {
    eprint!("{prompt}");
    io::stderr().flush()?;
    #[cfg(unix)]
    {
        read_no_echo_unix()
    }
    #[cfg(not(unix))]
    {
        read_line_trimmed()
    }
}

fn read_line_trimmed() -> io::Result<String> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

#[cfg(unix)]
fn read_no_echo_unix() -> io::Result<String> {
    use std::io::IsTerminal;
    if !io::stdin().is_terminal() {
        return read_line_trimmed();
    }
    let fd = libc::STDIN_FILENO;
    // SAFETY: tcgetattr writes `original` before any read. On failure we return without reading it.
    let mut original: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
        return read_line_trimmed();
    }
    let mut hidden = original;
    hidden.c_lflag &= !libc::ECHO;
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) } != 0 {
        return read_line_trimmed();
    }
    let result = read_line_trimmed();
    // SAFETY: `original` is the termios tcgetattr read from this stdin fd.
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &original);
    }
    eprintln!();
    result
}
