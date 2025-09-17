use std::collections::BTreeMap;

use crate::search_param as aidbox;
use miette::Diagnostic;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Serialize, Clone)]
pub struct SearchParameter {
    pub url: String,
    pub name: String,
    pub description: String,
    pub status: SearchParameterStatus,
    pub code: String,
    pub base: Vec<String>,
    pub r#type: SearchParameterType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Vec<String>>,
    pub expression: String,
}

#[derive(Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SearchParameterType {
    Composite,
    Date,
    Number,
    Quantity,
    Reference,
    Resource,
    Special,
    String,
    Token,
    Uri,
}

#[derive(Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SearchParameterStatus {
    Draft,
    Active,
    Retired,
    Unknown,
}

impl From<aidbox::SearchParameterType> for SearchParameterType {
    fn from(value: aidbox::SearchParameterType) -> Self {
        match value {
            aidbox::SearchParameterType::Composite => SearchParameterType::Composite,
            aidbox::SearchParameterType::Date => SearchParameterType::Date,
            aidbox::SearchParameterType::Number => SearchParameterType::Number,
            aidbox::SearchParameterType::Quantity => SearchParameterType::Quantity,
            aidbox::SearchParameterType::Reference => SearchParameterType::Reference,
            aidbox::SearchParameterType::String => SearchParameterType::String,
            aidbox::SearchParameterType::Token => SearchParameterType::Token,
            aidbox::SearchParameterType::Uri => SearchParameterType::Uri,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("The filter {} is too complex", serde_json::to_string(filter).expect("serde_json serialization fails only on non-string keys. We have string keys"))]
    TooComplexFilter { filter: BTreeMap<String, Value> },
}

fn escape_fhirpath_string(s: &str) -> String {
    s.replace('"', "\"\"").replace('\'', "\\\"")
}

fn filter_to_expression(filter: &BTreeMap<String, Value>) -> Result<String, Error> {
    let vals: Result<Vec<String>, Error> = filter
        .iter()
        .filter_map(|(k, v)| {
            let v = match v {
                Value::Null => return None,
                Value::Bool(_) => String::from("true"),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.to_owned(),
                Value::Array(_) | Value::Object(_) => {
                    return Some(Err(Error::TooComplexFilter {
                        filter: filter.to_owned(),
                    }));
                }
            };
            Some(Ok(format!("{k}='{}'", escape_fhirpath_string(&v))))
        })
        .collect();

    Ok(format!("where({})", vals?.join(" and ")))
}

pub fn convert(aidbox_sp: &aidbox::SearchParameter) -> Result<SearchParameter, Error> {
    let sp_url_component = match &aidbox_sp.id {
        Some(id) => format!("id-{}", id),
        None => format!("gen-{}-{}", aidbox_sp.resource.id, aidbox_sp.name),
    };

    let sp = SearchParameter {
        url: format!(
            "http://fhir.example.org/fhir/SearchParameter/{}",
            sp_url_component
        ),
        name: aidbox_sp.name.to_owned(),
        description: String::from("Auto-converted from Aidbox SearchParameter resource"),
        status: SearchParameterStatus::Active,
        code: aidbox_sp.name.to_owned(),
        base: vec![aidbox_sp.resource.id.to_owned()],
        r#type: aidbox_sp.r#type.into(),
        target: aidbox_sp.target.to_owned(),
        expression: aidbox_sp
            .expression
            .iter()
            .map(|expression| {
                let components: Result<Vec<String>, Error> = expression
                    .iter()
                    .map(|path_item| {
                        use aidbox::SearchParameterExpressionItem::*;
                        match path_item {
                            Path(path) => Ok(path.to_owned()),
                            Filter(filter) => Ok(filter_to_expression(filter)?),
                            // FIXME: WRONG expressin: a.0.b
                            Index(i) => Ok(i.to_string()),
                        }
                    })
                    .collect();
                Ok(components?.join("."))
            })
            .collect::<Result<Vec<String>, Error>>()?
            .join(" and "),
    };

    println!("{}", serde_json::to_string_pretty(&sp).unwrap());

    Ok(sp)
}
