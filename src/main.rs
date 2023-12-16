mod word_search;
mod kanji_search;
mod aux;
use std::{
    io::{stdin, stdout, Write},
    process::{Command, Stdio},
    env,
};

use word_search::word_search;
use kanji_search::search_by_radical;


use argparse::{ArgumentParser, List, Print, Store, StoreTrue};
use serde_json::Value;
use atty::Stream;

macro_rules! JISHO_URL {
    () => {
        "https://jisho.org/api/v1/search/words?keyword={}"
    };
}

}

fn main() -> Result<(), ureq::Error> {

    let term_size = if atty::is(Stream::Stdout) {
        terminal_size().unwrap_or(0)
    } else {
        0
    };

    let options = parse_args();

    let mut query = String::new();
    loop {

        query.clear();
        if options.interactive {
            while query.trim().is_empty() || query.trim() == ":" || query.trim() == "：" {
                query.clear();
                print!("=> ");
                stdout().flush().unwrap();
                if (stdin().read_line(&mut query).expect("Can't read from stdin")) == 0 {
                    /* Exit on EOF */
                    return Ok(());
                }
            }
        } else {
            query = options.query.clone();
            if query.trim().is_empty() || query.trim() == ":" || query.trim() == "：" {
                return Ok(());
            }
        }
        query = query.trim().to_string();

        let mut lines_output = 0;
        let mut output = String::with_capacity(5242880); /* Give output 5MiB of buffer; Should be enough to avoid reallocs*/

        if query.starts_with(':') || query.starts_with('：') { /* Kanji search */
            search_by_radical(&mut query);
        } else {
        } else { /* Word search */
            // Do API request
            let body: Value = ureq::get(&format!(JISHO_URL!(), query))
                .call()?
                .into_json()?;

            if let Some(r) = word_search(&options, body, &query, &mut output) {
                lines_output += r;
            } else {
                eprintln!("Error: invalid json returned");
                return Ok(());
            }

        }
        if lines_output >= term_size - 1 && term_size != 0 {
            /* Output is a different process that is not a tty (i.e. less), but we want to keep colour */
            env::set_var("CLICOLOR_FORCE", "1");
            pipe_to_less(output);
        } else {
            print!("{}", output);
        }
        if !options.interactive {
            break;
        }
    }
    Ok(())
}

fn parse_args() -> aux::Options {
    let mut options = aux::Options::default();
    let mut query_vec: Vec<String> = Vec::new();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Use jisho.org from cli. \
                            Searching for kanji by radicals is also available if the radkfile file is installed in \"~/.local/share\" \
                            or \"~\\AppData\\Local\\\" if you're on Windows.");
        ap.add_option(
            &["-V", "--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version",
        );
        ap.refer(&mut options.limit).add_option(
            &["-n", "--limit"],
            Store,
            "Limit the amount of results",
        );
        ap.refer(&mut query_vec)
            .add_argument("Query", List, "Search terms using jisho.org;
                          Prepend it with ':' to search a kanji by radicals instead \
                          and ':*' to search a radical by strokes (e.g. ':口*').");

        ap.refer(&mut options.interactive).add_option(
            &["-i", "--interactive"],
            StoreTrue,
            "Don't exit after running a query",
        );

        ap.parse_args_or_exit();
    }

    options.query = query_vec.join(" ");
    options
}

fn pipe_to_less(output: String) {

    let command = Command::new("less")
                    .arg("-R")
                    .stdin(Stdio::piped())
                    .spawn();

    match command {
        Ok(mut process) => {
            if let Err(e) = process.stdin.as_ref().unwrap().write_all(output.as_bytes()) {
                panic!("couldn't pipe to less: {}", e);
            }

            /* We don't care about the return value, only whether wait failed or not */
            if process.wait().is_err() {
                panic!("wait() was called on non-existent child process\
                 - this should not be possible");
            }
        }

        /* less not found in PATH; print normally */
        Err(_e) => print!("{}", output)
    };
}

/* OS specific part of the program */
#[cfg(unix)]
fn terminal_size() -> Result<usize, i16> {
    use libc::{ioctl, STDOUT_FILENO, TIOCGWINSZ, winsize};

    unsafe {
        let mut size = winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size as *mut _) == 0 {
            Ok(size.ws_row as usize)
        } else {
            Err(-1)
        }
    }
}

#[cfg(windows)]
fn terminal_size() -> Result<usize, i16> {
    use windows_sys::Win32::System::Console::*;
    if let Err(e) = control::set_virtual_terminal(true) {
        panic!("Could not set terminal as virtual: {:?}", e);
    }

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE) as windows_sys::Win32::Foundation::HANDLE;

        let mut window = CONSOLE_SCREEN_BUFFER_INFO {
            dwSize: COORD { X: 0, Y: 0},
            dwCursorPosition: COORD { X: 0, Y: 0},
            wAttributes: 0,
            dwMaximumWindowSize: COORD {X: 0, Y: 0},
            srWindow: SMALL_RECT {
                Top: 0,
                Bottom: 0,
                Left: 0,
                Right: 0
            }
        };
        if GetConsoleScreenBufferInfo(handle, &mut window) == 0 {
            Err(0)
        } else {
            Ok((window.srWindow.Bottom - window.srWindow.Top) as usize)
        }
    }
}
