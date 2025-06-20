pub mod attribute;
pub mod paths;
pub mod trie;

use std::{
    io::BufReader,
    path::{Path, PathBuf},
};

use clap::Parser;
use walkdir::WalkDir;

/// Generate structure definition from Aidbox attributes
#[derive(Debug, Parser)]
#[command(arg_required_else_help = true)]
struct Args {
    /// path with Attribute files
    path: PathBuf,
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

fn main() -> Result<(), String> {
    let mut had_errors = false;
    let args = Args::parse();
    let path = args.path;

    let walker = WalkDir::new(path).into_iter();

    let mut aidbox_attributes: Vec<attribute::aidbox::Attribute> = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("{}", error);
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
                eprintln!("{}", error);
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
                eprintln!("{}", error);
                continue;
            }
        };

        aidbox_attributes.push(aidbox_attribute);
    }

    let mut typed_attributes: Vec<attribute::typed::Attribute> = Vec::new();

    for aidbox_attribute in aidbox_attributes {
        let (typed_attribute, errors) = attribute::typed::Attribute::build_from(aidbox_attribute);

        if !errors.is_empty() {
            had_errors = true;
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
        eprintln!("{}", error);
    }

    let (inverted_forest, errors) = trie::inverted::Forest::build_from(extension_separated_forest);
    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    let (exts, errors) = trie::fhir::collect_extensions(inverted_forest);

    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    for ext in exts {
        println!("{}", serde_json::to_string_pretty(&ext).unwrap());
    }

    if had_errors {
        Err("Error".to_owned())
    } else {
        Ok(())
    }
}
