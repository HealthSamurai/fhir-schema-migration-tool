pub mod attribute;
pub mod builtin;
pub mod paths;
pub mod resource_map;
pub mod search_param;
pub mod trie;

use flate2::{Compression, write::GzEncoder};
use miette::Diagnostic;
use serde_json::json;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process,
};

use clap::{Parser, ValueEnum};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{search_param::SearchParameter, trie::fhir::StructureDefinition};

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

    /// Target FHIR version.
    #[arg(short, long, value_enum)]
    fhir_version: FhirVersion,

    /// Target IG package file (ex. fce.tgz). If not specified, all resources are written to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Exclude type from generating (e.g. for custom resources).
    #[arg(short, long)]
    exclude: Vec<String>,
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
    Walk {
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
        source: serde_json::Error,
    },

    #[error("Could not read {filename} as Aidbox search parameter")]
    BadSearchParameter {
        filename: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("Could not parse {filename} as JSON")]
    BadJson {
        filename: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("Could not parse {filename} as YAML")]
    BadYaml {
        filename: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Not allowed target resource type {resource_type}")]
    NotAllowedTargetResource { resource_type: String },

    #[error("Not supported resource type {resource_type} in {filename}")]
    NotSupportedResourceType {
        filename: PathBuf,
        resource_type: String,
    },

    #[error("Missing resource type in {filename}")]
    MissingResourceType { filename: PathBuf },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FhirVersion {
    #[value(name = "4.0.0")]
    V4_0_0,
    #[value(name = "4.0.1")]
    V4_0_1,
    #[value(name = "4.3.0")]
    V4_3_0,
    #[value(name = "5.0.0")]
    V5_0_0,
}

pub fn make_package_json(fhir_version: FhirVersion) -> String {
    let version_string: &'static str = match fhir_version {
        FhirVersion::V4_0_0 => "4.0.0",
        FhirVersion::V4_0_1 => "4.0.1",
        FhirVersion::V4_3_0 => "4.3.0",
        FhirVersion::V5_0_0 => "5.0.0",
    };

    let pkg_name: &'static str = match fhir_version {
        FhirVersion::V4_0_0 => "hl7.fhir.r4.core",
        FhirVersion::V4_0_1 => "hl7.fhir.r4.core",
        FhirVersion::V4_3_0 => "hl7.fhir.r4b.core",
        FhirVersion::V5_0_0 => "hl7.fhir.r5.core",
    };

    serde_json::to_string_pretty(&json!({
        "name": "legacy-fce.aidbox",
        "version": "0.0.0",
        "type": "IG",
        "dependencies": {
            pkg_name: version_string
        }
    }))
    .unwrap()
}

pub fn make_package(
    output: PathBuf,
    exts: Vec<StructureDefinition>,
    profiles: Vec<StructureDefinition>,
    fhir_version: FhirVersion,
) -> anyhow::Result<()> {
    let file = File::create(output)?;
    let gzip = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(gzip);

    {
        let package_json = make_package_json(fhir_version);
        let mut header = tar::Header::new_gnu();
        header.set_size(package_json.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
        );
        header.set_cksum();
        tar.append_data(&mut header, "package/package.json", package_json.as_bytes())?;
    }

    for (i, ext) in exts.into_iter().enumerate() {
        let name = format!(
            "package/StructureDefinition-Extension-{}-{}.json",
            &ext.name, i
        );
        let sd = serde_json::to_string_pretty(&ext).expect("Bug: invalid genereated SD");

        let mut header = tar::Header::new_gnu();
        header.set_size(sd.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
        );
        header.set_cksum();
        tar.append_data(&mut header, name, sd.as_bytes())?;
    }

    for (i, profile) in profiles.into_iter().enumerate() {
        let name = format!("package/StructureDefinition-{}-{}.json", &profile.name, i);
        let sd = serde_json::to_string_pretty(&profile).expect("Bug: invalid genereated SD");

        let mut header = tar::Header::new_gnu();
        header.set_size(sd.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
        );
        header.set_cksum();
        tar.append_data(&mut header, name, sd.as_bytes())?;
    }

    let gzip = tar.into_inner()?;
    let _file = gzip.finish()?;

    Ok(())
}

fn read_file(path: &Path) -> Result<serde_json::Value, Error> {
    let file = std::fs::File::open(path).map_err(|error| Error::ReadFile {
        filename: path.to_owned(),
        source: error,
    })?;
    let file = BufReader::new(file);
    if is_json(path) {
        serde_json::from_reader(file).map_err(|error| Error::BadJson {
            filename: path.to_owned(),
            source: error,
        })
    } else {
        serde_yaml::from_reader(file).map_err(|error| Error::BadYaml {
            filename: path.to_owned(),
            source: error,
        })
    }
}

#[derive(Debug)]
enum Data {
    Attribute(Box<attribute::aidbox::Attribute>),
    SearchParameter(SearchParameter),
}

fn read_data(path: &Path) -> Result<Data, Error> {
    let raw_data: serde_json::Value = read_file(path)?;
    match raw_data["resourceType"].as_str() {
        Some("Attribute") => serde_json::from_value::<attribute::aidbox::Attribute>(raw_data)
            .map(|attrs| Data::Attribute(Box::new(attrs)))
            .map_err(|error| Error::BadAttribute {
                filename: path.to_owned(),
                source: error,
            }),
        Some("SearchParameter") => {
            serde_json::from_value::<search_param::SearchParameter>(raw_data)
                .map(Data::SearchParameter)
                .map_err(|error| Error::BadSearchParameter {
                    filename: path.to_owned(),
                    source: error,
                })
        }
        Some(resource_type) => Err(Error::NotSupportedResourceType {
            filename: path.to_path_buf(),
            resource_type: (resource_type.to_owned()),
        }),
        None => Err(Error::MissingResourceType {
            filename: path.to_owned(),
        }),
    }
}

fn main() {
    // println!("{:#?}", get_builtin_resources(FhirVersion::V4_0_1));
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
    let mut aidbox_search_params: Vec<search_param::SearchParameter> = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                had_errors = true;
                eprintln!(
                    "{:?}",
                    miette::Report::new(Error::Walk {
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

        match read_data(path) {
            Ok(Data::Attribute(data)) => {
                aidbox_attributes.push(*data);
            }
            Ok(Data::SearchParameter(data)) => {
                aidbox_search_params.push(data);
            }
            Err(error) => {
                had_errors = true;
                eprintln!("{:?}", miette::Report::new(error));
            }
        }
    }

    let mut all_attributes = aidbox_attributes.clone();
    all_attributes.extend(builtin::get_builtin_resources(args.fhir_version).attribute);

    for aidbox_sp in aidbox_search_params {
        let _ = search_param::fhir::convert(&all_attributes, &aidbox_sp);
    }

    let mut typed_attributes: Vec<attribute::typed::Attribute> = Vec::new();

    for aidbox_attribute in aidbox_attributes {
        if aidbox_attribute.resource.resource_type == "Entity"
            && args.exclude.contains(&aidbox_attribute.resource.id)
        {
            continue;
        } else if aidbox_attribute.resource.resource_type == "Entity"
            && !resource_map::is_known_type(&aidbox_attribute.resource.id)
        {
            had_errors = true;
            eprintln!(
                "{:?}",
                miette::Report::new(Error::NotAllowedTargetResource {
                    resource_type: aidbox_attribute.resource.id.clone()
                })
            )
        }

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
        if let Some(out_file) = args.output {
            match make_package(out_file, exts, profiles, args.fhir_version) {
                Ok(_) => (),
                Err(error) => {
                    eprintln!("{:?}", error);
                    process::exit(1)
                }
            };
        } else {
            for ext in exts {
                println!("{}", serde_json::to_string_pretty(&ext).unwrap());
            }
            for profile in profiles {
                println!("{}", serde_json::to_string_pretty(&profile).unwrap());
            }
        }
    }

    if had_errors {
        process::exit(1);
    }
}
