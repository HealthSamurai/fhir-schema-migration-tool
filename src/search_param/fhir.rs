use std::{
    collections::{BTreeMap, HashMap},
    vec,
};

use crate::{
    attribute::aidbox::Attribute,
    search_param::{self as aidbox},
};
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
    #[error("The filter {} is too complex",
        serde_json::to_string(filter).expect("serde_json serialization fails only on non-string keys. We have string keys"))]
    TooComplexFilter { filter: BTreeMap<String, Value> },

    #[error("Enum attribute not implemented for Aidbox Search Parameters {}",
        serde_json::to_string(expression).expect("serde_json serialization fails only on non-string keys. We have string keys"))]
    EnumAttributeNotImplemented {
        expression: aidbox::SearchParameterExpression,
    },
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

fn convert_path(
    resource_type: String,
    attributes: &[Attribute],
    expr: &aidbox::SearchParameterExpression,
) -> Result<String, Error> {
    use aidbox::SearchParameterExpressionItem::*;
    let mut res = resource_type.to_owned();

    let mut prefix: Vec<String> = Vec::new();

    for item in expr {
        let item = match item {
            Path(item) => {
                res.push('.');
                item
            }
            Filter(filter) => {
                res.push('.');
                res.push_str(&filter_to_expression(filter)?);
                continue;
            }
            Index(i) => {
                res.push('[');
                res.push_str(&i.to_string());
                res.push(']');
                continue;
            }
        };

        prefix.push(item.to_owned());

        let Some(attribute) = attributes.iter().find(|attr| attr.path == prefix) else {
            res.push_str(item);
            continue;
        };

        if attribute.r#enum.is_some() {
            return Err(Error::EnumAttributeNotImplemented {
                expression: expr.to_owned(),
            });
        }

        if let Some(ext_url) = &attribute.extension_url {
            res.push_str(&format!("extension('{}')", escape_fhirpath_string(ext_url)));
            if let Some(target) = &attribute.r#type {
                res.push_str(&format!(".value.ofType({})", target.id));
            }
        } else {
            res.push_str(item)
        }
    }
    Ok(res)
}

pub fn convert(
    attributes: &Vec<Attribute>,
    aidbox_sp: &aidbox::SearchParameter,
) -> Result<SearchParameter, Error> {
    let mut resource_type_to_attributes = HashMap::<String, Vec<Attribute>>::new();
    for attribute in attributes {
        resource_type_to_attributes
            .entry(attribute.resource.id.to_owned())
            .or_default()
            .push(attribute.clone());
    }

    let sp_url_component = match &aidbox_sp.id {
        Some(id) => format!("id-{}", id),
        None => format!("gen-{}-{}", aidbox_sp.resource.id, aidbox_sp.name),
    };

    let no_attributes: Vec<Attribute> = vec![];
    let expression = aidbox_sp
        .expression
        .iter()
        .map(|expression| {
            convert_path(
                aidbox_sp.resource.id.to_owned(),
                resource_type_to_attributes
                    .get(&aidbox_sp.resource.id)
                    .unwrap_or(&no_attributes),
                expression,
            )
        })
        .collect::<Result<Vec<String>, Error>>()?
        .join(" or ");

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
        expression,
    };

    println!("{}", serde_json::to_string_pretty(&sp).unwrap());

    Ok(sp)
}
