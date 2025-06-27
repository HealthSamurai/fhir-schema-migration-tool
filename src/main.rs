pub mod attribute;
pub mod paths;
pub mod trie;

use miette::{Diagnostic, miette};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process,
};

use clap::Parser;
use thiserror::Error;
use walkdir::WalkDir;

/// Generate structure definition from Aidbox attributes
#[derive(Debug, Parser)]
#[command(arg_required_else_help = true)]
struct Args {
    /// Path to Attribute files
    path: PathBuf,

    /// Try to generate StructureDefinition resources even if there were errors
    #[arg(long)]
    ignore_errors: bool,

    /// Ignore errors related to isSummary, isModifier, order flags
    #[arg(long)]
    ignore_flags: bool,
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

fn is_json_or_yaml(path: &Path) -> bool {
    is_json(path) || is_yaml(path)
}

#[derive(Debug, Error, Diagnostic)]
enum Error {
    #[error("Error while searching for JSON and YAML files in {base_path}")]
    #[diagnostic(help("Ensure the directory name is correct and you have access rights"))]
    WalkError {
        base_path: PathBuf,
        #[source]
        source: walkdir::Error,
    },

    #[error("Could not read contents of the file {filename}")]
    ReadFile {
        filename: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Could not read {filename} as Aidbox attribute")]
    BadAttribute {
        filename: PathBuf,
        #[source]
        source: attribute::aidbox::Error,
    },
}

fn main() {
    _ = miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .break_words(true)
                .width(120)
                .with_cause_chain()
                .build(),
        )
    }));

    let mut had_errors = false;
    let args = Args::parse();
    let path = args.path;

    let walker = WalkDir::new(&path).into_iter();

    let mut aidbox_attributes: Vec<attribute::aidbox::Attribute> = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                had_errors = true;
                eprintln!(
                    "{:?}",
                    miette::Report::new(Error::WalkError {
                        base_path: path.clone(),
                        source: error
                    })
                );
                continue;
            }
        };

        let path = entry.path();
        if !is_json_or_yaml(path) {
            continue;
        }
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) => {
                had_errors = true;
                eprintln!(
                    "{:?}",
                    miette::Report::new(Error::ReadFile {
                        filename: path.to_owned(),
                        source: error
                    })
                );
                continue;
            }
        };
        let file = BufReader::new(file);

        let aidbox_attribute = if is_json(path) {
            attribute::aidbox::Attribute::from_json(file)
        } else {
            attribute::aidbox::Attribute::from_yaml(file)
        };
        let aidbox_attribute = match aidbox_attribute {
            Ok(attribute) => attribute,
            Err(error) => {
                had_errors = true;
                eprintln!(
                    "{:?}",
                    miette::Report::new(Error::BadAttribute {
                        filename: path.to_owned(),
                        source: error
                    })
                );
                continue;
            }
        };

        aidbox_attributes.push(aidbox_attribute);
    }

    let mut typed_attributes: Vec<attribute::typed::Attribute> = Vec::new();

    for aidbox_attribute in aidbox_attributes {
        let (typed_attribute, errors) = attribute::typed::Attribute::build_from(aidbox_attribute);

        let errors = if args.ignore_flags {
            errors
                .into_iter()
                .filter(|error| {
                    !matches!(
                        error.source,
                        attribute::typed::InvalidAttributeError::SummaryPresent
                            | attribute::typed::InvalidAttributeError::ModifierPresent
                            | attribute::typed::InvalidAttributeError::OrderPresent
                    )
                })
                .collect()
        } else {
            errors
        };

        if !errors.is_empty() {
            had_errors = true;
        }

        for error in errors {
            eprintln!("{:?}", miette::Report::new(error))
        }

        let Some(typed_attribute) = typed_attribute else {
            continue;
        };

        typed_attributes.push(typed_attribute);
    }

    let (raw_forest, errors) = trie::raw::Forest::build_from_attributes(&typed_attributes);
    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    let path_forest = trie::path::Forest::build_from(raw_forest);
    let (extension_separated_forest, errors) =
        trie::extension_separated::Forest::build_from(path_forest);

    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{:?}", miette::Report::new(error))
    }

    let (inverted_forest, errors) = trie::inverted::Forest::build_from(extension_separated_forest);
    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    let profiles = trie::fhir::make_profiles(&inverted_forest);

    let (exts, errors) = trie::fhir::collect_extensions(inverted_forest);

    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    if !had_errors || args.ignore_errors {
        for ext in exts {
            println!("{}", serde_json::to_string_pretty(&ext).unwrap());
        }
        for profile in profiles {
            println!("{}", serde_json::to_string_pretty(&profile).unwrap());
        }
    }

    if had_errors {
        process::exit(1);
    }
}
