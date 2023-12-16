use crate::aux::*;

use serde_json::Value;
use colored::*;

pub fn sentence_search(options: &Options, body: Value, output: &mut String) -> Result<usize, i8>{

    let mut lines = 0;
    let body = value_to_arr({
        let body = body.get("results");

        if body.is_none() {
            return Err(-1);
        }

        body.unwrap()
    });

    let mut i = 1;

    /* 
     * Each entry is an english or japanese sentence and we're pairing it up with the equivalents in
     * the translations array
     */
    for entry in body.iter() {
        if i >= options.limit && options.limit != 0 {
            break;
        }

        let translations = value_to_arr({
            let translations = entry.get("translations");

            if translations.is_none() {
                return Err(-1);
            }
            let translations = value_to_arr(translations.unwrap()).get(0);
            if translations.is_none() {
                return Err(-1);
            }
            translations.unwrap()
        });


        for translation in translations.iter() {
            let index_str = format!("{}.", i).bright_black();
            
            /* prefer to keep japanese sentences on top */
            if entry.get("lang").unwrap() == "eng" {
                *output += &format!("{} {}\n   {}\n\n", index_str, value_to_str(translation.get("text").unwrap()).replace("\"", ""), value_to_str(entry.get("text").unwrap()).replace("\"", ""));
            } else {
                *output += &format!("{} {}\n   {}\n\n", index_str, value_to_str(entry.get("text").unwrap()).replace("\"", ""), value_to_str(translation.get("text").unwrap()).replace("\"", ""));
            }

            i += 1;
            lines += 3;
        }

    }
    if !output.is_empty() {
        return Ok(lines)
    }
    Err(1)
}
