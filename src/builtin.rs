use std::collections::BTreeMap;

use flate2::bufread::GzDecoder;
use serde::Deserialize;

use crate::{FhirVersion, attribute::aidbox, search_param::SearchParameter};

const FHIR_4_0_0: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fhir-4.0.0.json.gz"
));

const FHIR_4_0_1: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fhir-4.0.1.json.gz"
));

const FHIR_4_3_0: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fhir-4.3.0.json.gz"
));

const FHIR_5_0_0: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fhir-5.0.0.json.gz"
));

#[derive(Debug, Clone, Deserialize)]

struct Collection {
    resources: CollectionResources,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CollectionResources {
    attribute: BTreeMap<String, aidbox::Attribute>,
    search_parameter: BTreeMap<String, SearchParameter>,
}

#[derive(Debug, Clone)]
pub struct BuiltinResources {
    pub attribute: Vec<aidbox::Attribute>,
    pub search_parameter: Vec<SearchParameter>,
}

pub fn get_builtin_resources(fhir_version: FhirVersion) -> BuiltinResources {
    let f = match fhir_version {
        FhirVersion::V4_0_0 => FHIR_4_0_0,
        FhirVersion::V4_0_1 => FHIR_4_0_1,
        FhirVersion::V4_3_0 => FHIR_4_3_0,
        FhirVersion::V5_0_0 => FHIR_5_0_0,
    };

    let decoder = GzDecoder::new(f);
    let collection: Collection =
        serde_json::from_reader(decoder).expect("Error in bundled Aidbox attributes");

    let attributes = collection.resources.attribute;
    let attributes: Vec<aidbox::Attribute> = attributes
        .into_iter()
        .map(|(id, mut attr)| {
            attr.id = Some(id);
            attr
        })
        .collect();

    let search_parameters = collection.resources.search_parameter;
    let search_parameters: Vec<SearchParameter> = search_parameters
        .into_iter()
        .map(|(id, mut param)| {
            param.id = Some(id);
            param
        })
        .collect();

    BuiltinResources {
        attribute: attributes,
        search_parameter: search_parameters,
    }
}
