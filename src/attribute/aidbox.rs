use std::io::Read;

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Entity attribute metadata
pub struct Attribute {
    /// Attribute id as stored in the database
    pub id: String,

    /// Path of the element this attribute is describing
    pub path: Vec<String>,

    /// module from which the attribute comes
    pub module: Option<String>,

    /// Some description?
    pub text: Option<String>,

    /// Another description?
    pub description: Option<String>,

    /// Resource type which this attribute describes
    pub resource: Reference,

    /// Target type if the attribute is describing primitive element
    pub r#type: Option<Reference>,

    /// Extension url if the attribute is describing first-class extension
    pub extension_url: Option<String>,

    /// JSON schema
    pub schema: Option<Value>,

    /// Is this element required?
    pub is_required: Option<bool>,

    /// Is this element an array (in Aidbox format)
    pub is_collection: Option<bool>,

    /// Are extra properties allowed?
    pub is_open: Option<bool>,

    /// If the element is polymorphic, which targets are allowed
    pub union: Option<Vec<Reference>>,

    /// Some uniqueness constraint?
    pub is_unique: Option<bool>,

    /// List of allowed values
    pub r#enum: Option<String>,

    /// I don't know
    pub order: Option<i64>,

    /// Is this a FHIR summary element?
    pub is_summary: Option<bool>,

    /// Is this a FHIR modifier extension?
    pub is_modifier: Option<bool>,

    /// ValueSet with allowed values
    pub value_set: Option<Reference>,

    /// If this is a reference, which targets are allowed
    pub refers: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not parse Attribute resource as JSON (malformed JSON or invalid resource)")]
    InvalidJson(#[from] serde_json::Error),

    #[error("Could not parse Attribute resource as YAML (malformed YAML or invalid resource)")]
    InvalidYaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub id: String,
    pub resource_type: String,
}

impl Attribute {
    pub fn from_json(reader: impl Read) -> Result<Self, Error> {
        serde_json::from_reader(reader).map_err(|e| e.into())
    }

    pub fn from_yaml(reader: impl Read) -> Result<Self, Error> {
        serde_yaml::from_reader(reader).map_err(|e| e.into())
    }
}
