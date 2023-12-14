mod kanji_search;
use std::{
    io::{stdin, stdout, Write},
    process::{Command, Stdio},
    error::Error,
    env,
};

use kanji_search::search_by_radical;

use argparse::{ArgumentParser, List, Print, Store, StoreTrue};
use colored::*;
use serde_json::Value;
use atty::Stream;

macro_rules! JISHO_URL {
    () => {
        "https://jisho.org/api/v1/search/words?keyword={}"
    };
}

#[derive(Default, Debug, Clone)]
struct Options {
    limit: usize,
    query: String,
    interactive: bool,
}

fn main() -> Result<(), Box<dyn Error>> {

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
        let mut output = String::with_capacity(5242880); /* Give output 5MB of buffer; Should be enough to avoid reallocs*/

        if query.starts_with(':') || query.starts_with('：') {
            search_by_radical(&mut query);
        } else {
            // Do API request
            let body: Value = ureq::get(&format!(JISHO_URL!(), query))
                .call()?
                .into_json()?;

            // Try to get the data json-object
            let body = value_to_arr({
                let body = body.get("data");

                if body.is_none() {
                    eprintln!("Error! Invalid response");
                    return Ok(());
                }

                body.unwrap()
            });

            if options.interactive {
                println!();
            }

            /* Iterate over meanings and print them */
            for (i, entry) in body.iter().enumerate() {
                if i >= options.limit && options.limit != 0 {
                    break;
                }
                if let Some(r) = print_item(&query, entry, &mut output) {
                    lines_output += r;
                }

                output.push('\n');
                lines_output += 1;
            }
            output.pop();
            lines_output = lines_output.saturating_sub(1);

            if lines_output >= term_size - 1 && term_size != 0 {
                /* Output is a different process that is not a tty (i.e. less), but we want to keep colour */
                env::set_var("CLICOLOR_FORCE", "1");
                pipe_to_less(output);
            } else {
                print!("{}", output);
            }


        }
        if !options.interactive {
            break;
        }
    }
    Ok(())
}

fn print_item(query: &str, value: &Value, output: &mut String) -> Option<usize> {
    let japanese = value_to_arr(value.get("japanese")?);
    let main_form = japanese.get(0)?;
    let mut num_of_lines = 0;

    *output += &format!("{} {}\n", format_form(query, main_form)?, format_result_tags(value));

    /* Print senses */
    let senses = value_to_arr(value.get("senses")?);
    let mut prev_parts_of_speech = String::new();

    for (i, sense) in senses.iter().enumerate() {
        let (sense_str, new_part_of_speech) = format_sense(sense, i, &mut prev_parts_of_speech);
        if !sense_str.is_empty() {
            /*
             * If the current meaning of our word is a different part of speech
             * (e.g. previous meaning was 'Noun' and the current is 'Adverb'), an extra line will be
             * printed with this information
             */
            if new_part_of_speech {
                num_of_lines += 1;
            }

            *output += &format!("    {}\n", sense_str);
        }
    }

    /* Print alternative readings and kanji usage */
    if let Some(form) = japanese.get(1) {
        num_of_lines += 2;

        *output += &format!("    {}", "Other forms\n".bright_blue());
        *output += &format!("    {}", format_form(query, form)?);

        for i in 2..japanese.len() {
            *output += &format!(", {}", format_form(query, japanese.get(i)?)?);
        }
        output.push('\n');
    }

    num_of_lines += senses.len() + 1;
    Some(num_of_lines)
}

fn format_form(query: &str, form: &Value) -> Option<String> {
    let reading = form
        .get("reading")
        .map(value_to_str)
        .unwrap_or(query);

    let word = value_to_str(form.get("word").unwrap_or(form.get("reading")?));

    Some(format!("{}[{}]", word, reading))
}

fn format_sense(value: &Value, index: usize, prev_parts_of_speech: &mut String) -> (String, bool) {
    let english_definitons = value.get("english_definitions");
    let parts_of_speech = value.get("parts_of_speech");
    if english_definitons.is_none() {
        return ("".to_owned(), false);
    }

    let english_definiton = value_to_arr(english_definitons.unwrap());

    let parts_of_speech = if let Some(parts_of_speech) = parts_of_speech {
        let parts = value_to_arr(parts_of_speech)
            .iter()
            .map(value_to_str)
            .collect::<Vec<&str>>()
            .join(", ");

        /* Do not repeat a meaning's part of speech if it is the same as the previous meaning */
        if !parts.is_empty() && parts != *prev_parts_of_speech {
            *prev_parts_of_speech = parts.clone();
            format!("{}\n    ", parts.bright_blue())
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let new_part_of_speech = !parts_of_speech.is_empty();

    let index_str = format!("{}.",(index + 1));
    let mut tags = format_sense_tags(value);
    let info = format_sense_info(value);

    if !info.is_empty() && !tags.is_empty() {
        tags.push(',');
    }

    (format!(
        "{}{} {}{}{}",
        parts_of_speech,
        index_str.bright_black(),
        english_definiton
            .iter()
            .map(value_to_str)
            .collect::<Vec<&str>>()
            .join(", "),
        tags.bright_black(),
        info.bright_black(),
    ), new_part_of_speech)
}

/// Format tags from a whole meaning
fn format_result_tags(value: &Value) -> String {
    let mut builder = String::new();

    let is_common_val = value.get("is_common");
    if is_common_val.is_some() && value_to_bool(is_common_val.unwrap()) {
        builder.push_str(&"(common) ".bright_green().to_string());
    }

    if let Some(jlpt) = value.get("jlpt") {
        /*
         * The jisho API actually returns an array of all of JLTP levels for each alternative of a word
         * Since the main one is always at index 0, we take that for formatting
         */
        let jlpt = value_to_arr(jlpt);
        if !jlpt.is_empty() {
            let jlpt = value_to_str(jlpt.get(0).unwrap())
                .replace("jlpt-", "")
                .to_uppercase();
            builder.push_str(&format!("({}) ", jlpt.bright_blue()));
        }
    }

    builder
}

/// Format tags from a single sense entry
fn format_sense_tags(value: &Value) -> String {
    let mut builder = String::new();

    if let Some(tags) = value.get("tags") {
        let tags = value_to_arr(tags);

        if let Some(tag) = tags.get(0) {
            let t = format_sense_tag(value_to_str(tag));
            builder += &format!(" {}", t.as_str());
        }

        for tag in tags.get(1).iter() {
            let t = format_sense_tag(value_to_str(tag));
            builder += &format!(", {}", t.as_str());
        }
    }
    builder
}

fn format_sense_tag(tag: &str) -> String {
    match tag {
        "Usually written using kana alone" => "UK".to_string(),
        s => s.to_string(),
    }
}

fn format_sense_info(value: &Value) -> String {
    let mut builder = String::new();
    if let Some(all_info) = value.get("info") {
        let all_info = value_to_arr(all_info);

        if let Some(info) = all_info.get(0) {
            builder += &format!(" {}", value_to_str(info));
        }

        for info in all_info.get(1).iter() {
            builder += &format!(", {}", value_to_str(info));
        }
    }
    builder
}

//
// --- Value helper
//

fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        _ => unreachable!(),
    }
}

fn value_to_str(value: &Value) -> &str {
    match value {
        Value::String(s) => s,
        _ => unreachable!(),
    }
}

fn value_to_arr(value: &Value) -> &Vec<Value> {
    match value {
        Value::Array(a) => a,
        _ => unreachable!(),
    }
}

fn parse_args() -> Options {
    let mut options = Options::default();
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
