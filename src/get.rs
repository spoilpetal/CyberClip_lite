use anyhow::{bail, Context, Result};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub struct GetResult {
    pub text: String,
    pub matches: usize,
}

pub fn search(path: &Path, query: &str, show_numbers: bool) -> Result<GetResult> {
    if query.is_empty() {
        bail!("Search query cannot be empty");
    }

    let file = File::open(path)
        .with_context(|| format!("Could not open {}", path.display()))?;

    let reader = BufReader::new(file);
    let mut output = String::new();
    let mut matches = 0;

    for (index, line) in reader.lines().enumerate() {
        let line = line?;

        if line.contains(query) {
            matches += 1;

            if show_numbers {
                output.push_str(&format!("{:>6}  {}\n", index + 1, line));
            } else {
                output.push_str(&line);
                output.push('\n');
            }
        }
    }

    Ok(GetResult {
        text: output,
        matches,
    })
}

pub fn extract(
    path: &Path,
    start: usize,
    end: usize,
    show_numbers: bool,
) -> Result<GetResult> {
    if start == 0 || end < start {
        bail!("Invalid line range");
    }

    let file = File::open(path)
        .with_context(|| format!("Could not open {}", path.display()))?;

    let reader = BufReader::new(file);
    let mut output = String::new();
    let mut matches = 0;

    for (index, line) in reader.lines().enumerate() {
        let number = index + 1;

        if number > end {
            break;
        }

        if number >= start {
            let line = line?;

            if show_numbers {
                output.push_str(&format!("{:>6}  {}\n", number, line));
            } else {
                output.push_str(&line);
                output.push('\n');
            }

            matches += 1;
        }
    }

    Ok(GetResult {
        text: output,
        matches,
    })
}


/// Read a complete UTF-8 text file without adding line numbers or changing its contents.
pub fn whole_file(path: &Path) -> Result<GetResult> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read text file {}", path.display()))?;
    let matches = text.lines().count();
    Ok(GetResult { text, matches })
}
