use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

pub mod fhir;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchParameter {
    /// Search Parameter ID
    pub id: Option<String>,

    /// Name of search parameter, used in search query string
    pub name: String,

    /// Module name
    pub module: Option<String>,

    /// Type of search parameter
    pub r#type: SearchParameterType,

    /// Reference to resource this search param attached to; like {id: 'Patient', resourceType: 'Entity'}
    pub resource: Reference,

    /// Reference target types
    pub target: Option<Vec<String>>,

    /// Searchable elements expression like [["telecom",{"system":"phone"}, "value"]]
    pub expression: Vec<SearchParameterExpression>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SearchParameterExpressionItem {
    Path(String),
    Index(usize),
    Filter(BTreeMap<String, Value>),
}

pub type SearchParameterExpression = Vec<SearchParameterExpressionItem>;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchParameterType {
    Composite,
    Date,
    Number,
    Quantity,
    Reference,
    String,
    Token,
    Uri,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub id: String,
    #[serde(rename = "resourceType")]
    pub resource_type: String,
}
