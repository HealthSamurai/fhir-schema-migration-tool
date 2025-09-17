use serde::Deserialize;
use serde_json::Value;

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
    Filter(serde_json::Map<String, Value>),
}

pub type SearchParameterExpression = Vec<SearchParameterExpressionItem>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchParameterType {
    String,
    Number,
    Date,
    Token,
    Quantity,
    Reference,
    Uri,
    Composite,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub id: String,
    #[serde(rename = "resourceType")]
    pub resource_type: String,
}
