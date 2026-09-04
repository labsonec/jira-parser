use std::{env, fs, io, io::Write, process};

use jira_parser::markdown_to_jira::MarkdownToJiraWarning;

enum InputSource {
    Text(String),
    File(String),
}

#[derive(Debug, PartialEq)]
enum OutputTarget {
    Stdout,
    File(String),
}

struct CliOptions {
    markdown_to_jira: bool,
    source: InputSource,
    output: OutputTarget,
}

fn usage() -> &'static str {
    "Usage: jira-parser [--markdown-to-jira] --text <TEXT> [--output <PATH|->]\n       jira-parser [--markdown-to-jira] -t <TEXT> [-o <PATH|->]\n       jira-parser [--markdown-to-jira] --file <PATH> [--output <PATH|->]\n       jira-parser [--markdown-to-jira] -f <PATH> [-o <PATH|->]"
}

fn parse_args<I>(mut args: I) -> Result<CliOptions, String>
where
    I: Iterator<Item = String>,
{
    let mut markdown_to_jira = false;
    let mut source = None;
    let mut output = None;

    while let Some(arg) = args.next() {
        let next_source = match arg.as_str() {
            "--markdown-to-jira" => {
                markdown_to_jira = true;
                continue;
            }
            "--text" | "-t" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {arg}"))?;
                InputSource::Text(value)
            }
            "--file" | "-f" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {arg}"))?;
                InputSource::File(value)
            }
            "--output" | "-o" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {arg}"))?;
                if output.is_some() {
                    return Err("output option may only be specified once".to_string());
                }
                output = Some(if value == "-" {
                    OutputTarget::Stdout
                } else {
                    OutputTarget::File(value)
                });
                continue;
            }
            _ => return Err(format!("unknown argument: {arg}")),
        };

        if source.is_some() {
            return Err("exactly one input source is required".to_string());
        }
        source = Some(next_source);
    }

    let source = source.ok_or_else(|| "exactly one input source is required".to_string())?;

    Ok(CliOptions {
        markdown_to_jira,
        source,
        output: output.unwrap_or(OutputTarget::Stdout),
    })
}

fn write_result(target: &OutputTarget, result: &str) -> Result<(), String> {
    match target {
        OutputTarget::Stdout => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(result.as_bytes())
                .and_then(|_| stdout.write_all(b"\n"))
                .map_err(|error| format!("failed to write stdout: {error}"))
        }
        OutputTarget::File(path) => fs::write(path, format!("{result}\n"))
            .map_err(|error| format!("failed to write {path}: {error}")),
    }
}

fn run() -> Result<(), String> {
    let options = parse_args(env::args().skip(1))?;
    let input = match options.source {
        InputSource::Text(text) => text,
        InputSource::File(path) => {
            fs::read_to_string(&path).map_err(|error| format!("failed to read {path}: {error}"))?
        }
    };

    if options.markdown_to_jira {
        let output = jira_parser::markdown_to_jira(&input)
            .map_err(|error| format!("failed to convert input: {}", error.message))?;
        write_result(&options.output, &output.markup)?;
        for warning in output.warnings {
            eprint_warning(&warning);
        }
    } else {
        let document = jira_parser::parse_jira(&input)
            .map_err(|error| format!("failed to parse input: {error}"))?;
        write_result(&options.output, &format!("{document:#?}"))?;
    }

    Ok(())
}

fn eprint_warning(warning: &MarkdownToJiraWarning) {
    eprintln!("warning[{}]: {}", warning.kind, warning.message);
    if let Some(source_excerpt) = &warning.source_excerpt {
        eprintln!("  source: {source_excerpt}");
    }
    if let Some(output_excerpt) = &warning.output_excerpt {
        eprintln!("  output: {output_excerpt}");
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        eprintln!("{}", usage());
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> std::vec::IntoIter<String> {
        values
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn output_defaults_to_stdout() {
        let options = parse_args(args(&["-t", "text"])).unwrap();

        assert_eq!(options.output, OutputTarget::Stdout);
    }

    #[test]
    fn dash_output_selects_stdout() {
        let options = parse_args(args(&["-t", "text", "--output", "-"])).unwrap();

        assert_eq!(options.output, OutputTarget::Stdout);
    }

    #[test]
    fn output_path_selects_file() {
        let options = parse_args(args(&["-t", "text", "-o", "result.txt"])).unwrap();

        assert_eq!(options.output, OutputTarget::File("result.txt".to_string()));
    }

    #[test]
    fn duplicate_output_is_rejected() {
        let error = parse_args(args(&["-t", "text", "-o", "one", "--output", "two"]))
            .err()
            .unwrap();

        assert_eq!(error, "output option may only be specified once");
    }

    #[test]
    fn missing_output_value_is_rejected() {
        let error = parse_args(args(&["-t", "text", "-o"])).err().unwrap();

        assert_eq!(error, "missing value for -o");
    }
}
