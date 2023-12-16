use crate::aux::*;

use serde_json::Value;
use colored::*;

pub fn word_search(options: &Options, body: Value, query: &String, mut output: &mut String) -> Option<usize> {
    let mut lines_output = 0;

    // Try to get the data json-object
    let body = value_to_arr({
        let body = body.get("data");

        if body.is_none() {
            return None;
        }

        body.unwrap()
    });

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

    Some(lines_output)
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
