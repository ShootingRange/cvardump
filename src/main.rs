extern crate clap;
extern crate csv;
extern crate regex;
extern crate tokio;
use clap::{App, Arg, SubCommand};
use csv::WriterBuilder;
use regex::RegexBuilder;
use std::cmp::Ordering;
use std::error::Error;
use std::io::{stdin, stdout, Read, Write};
use std::process::exit;
use std::ops::Index;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new("cvardump")
        .version("v1.0.0")
        .about("Dumps a list of cvars from Source engine into a CSV spreadsheet")
        .subcommand(
            SubCommand::with_name("rcon")
                .about("Connect to Source engine server using RCON to retrieve a list of cvars using the \"cvarlist\" command")
                .arg(
                    Arg::with_name("host")
                        .help("Server address and port, ex: 192.168.1.100:27015")
                        .required(true)
                        .index(1))
                .arg(
                    Arg::with_name("password")
                        .help("RCON password")
                        .required(true)
                        .index(2)
                )
        )
        .subcommand(
            SubCommand::with_name("manual")
                .help("Reads the output of \"cvarlist\" from a file. This option is useful for extracting cvars from Source engine clients")
                .arg(
                    Arg::with_name("input")
                        .help("Input file, default to reading from stdin")
                        .index(1)
                )
        )
        .arg(
            Arg::with_name("output")
                .help("Output file path, default to printing to the terminal")
                .long("output")
                .short("o")
                .global(true)
                .takes_value(true)
        );

    let matches = app.clone().get_matches();

    let input = match matches.subcommand_name() {
        None => {
            app.print_long_help()?;
            exit(0);
        }
        Some("rcon") => {
            let subcmd_matches = matches.subcommand().1.unwrap();
            let host = subcmd_matches.value_of("host").expect("missing required argument");
            let password = subcmd_matches
                .value_of("password")
                .expect("missing required argument");

            let mut conn = rcon::Connection::connect(host, password).await?;

            conn.cmd("cvarlist").await?
        }
        Some("manual") => {
            let subcmd_matches = matches.subcommand().1.unwrap();

            let mut input = String::new();
            match subcmd_matches.value_of("input") {
                // Default to reading from stdin/terminal
                None => match stdin().read_to_string(&mut input) {
                    Ok(_) => input,
                    Err(err) => {
                        eprintln!("Failed to read input from stdin\n\n{}", err);
                        exit(1);
                    }
                },
                Some(path) => match std::fs::read_to_string(path) {
                    Ok(input) => input,
                    Err(err) => {
                        eprintln!("Failed to read input from file\n\n{}", err);
                        exit(1);
                    }
                },
            }
        }
        Some(_) => unreachable!(),
    };

    let subcmd_matches = matches.subcommand().1.unwrap();
    let output: Box<dyn Write> = match subcmd_matches.value_of("output") {
        // Default to writing to stdout/terminal
        None => Box::new(stdout()),
        Some(path) => match std::fs::File::create(path) {
            Ok(file) => Box::new(file),
            Err(err) => {
                eprintln!("Failed to open output file\n\n{}", err);
                exit(1);
            }
        },
    };

    // Extract cvars from raw format
    let (cvars, expected_lines) = extract_cvars(input);
    if let Some(expected_lines) = expected_lines {
        match cvars.len().cmp(&expected_lines) {
            Ordering::Less => eprintln!(
                "[WARNING] Extracted less cvars than the number of cvars reported by \"cvarlist\""
            ),
            Ordering::Equal => {}
            Ordering::Greater => eprintln!(
                "[WARNING] Extracted more cvars than the number of cvars reported by \"cvarlist\""
            ),
        }
    }

    // Write cvar list to csv file
    write_cvar_csv(cvars, output)?;

    Ok(())
}

struct Cvar {
    name: String,
    default: String,
    attributes: Vec<String>,
    description: String,
}

/// Takes the output of `cvarlist` and parses the lines for cvars.
/// Ignored lines not matching a table entry.
fn extract_cvars(lines: String) -> (Vec<Cvar>, Option<usize>) {
    let regex_cvar = RegexBuilder::new(r#"^(.*?)\s*: (.*?)\s*: (.*?)\s*:(?: (.*)|)$"#)
        .build()
        .expect("Failed to compile regex");

    let regex_count = RegexBuilder::new(r#"^(\d+) total convars/concommands$"#)
        .build()
        .expect("Failed to compile regex");

    // Matches individual attributes from the attribute column
    let regex_attrs = RegexBuilder::new(r#", "(.*?)""#)
        .build()
        .expect("Failed to compile regex");

    // List of cvar that's gonna be build
    let mut cvars = Vec::new();
    // The number of cvars as reported by Source engine, if a count is found
    let mut expected_cvars: Option<usize> = Option::None;
    for line in lines.lines() {
        if let Some(captures) = regex_cvar.captures(line) {
            // Description is optional
            let description = match captures.get(4) {
                None => "",
                Some(description) => description.as_str()
            };

            // extract attributes
            let attrs: Vec<String> = regex_attrs.find_iter(captures.index(3))
                .map(|matches| {
                    regex_attrs.captures(matches.as_str()).unwrap().index(1).to_string()
                })
                .collect();

            cvars.push(Cvar {
                name: captures.index(1).to_string(),
                default: captures.index(2).to_string(),
                attributes: attrs,
                description: description.to_string()
            })
        } else if let Some(captures) = regex_count.captures(line) {
            if let Some(_) = expected_cvars {
                panic!("found cvar count twice");
            }

            // The count is always a non-empty sequence of digits, and should there for always be parsable into a integer
            expected_cvars = Some(captures.index(1).parse().unwrap());
        }
    }

    (cvars, expected_cvars)
}

fn write_cvar_csv(cvars: Vec<Cvar>, output: Box<dyn Write>) -> Result<(), Box<dyn Error>> {
    let mut wtr = WriterBuilder::new().from_writer(output);

    // Write columns headers
    wtr.write_record(vec![
        "name", "default", "attribtues", "description"
    ])?;

    for cvar in cvars {
        let record = vec![
            cvar.name,
            cvar.attributes.join(","),
            cvar.default,
            cvar.description,
        ];

        wtr.write_record(&record)?;
    }

    Ok(())
}
