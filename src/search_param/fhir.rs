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
    s.replace(r#"\"#, r#"\\"#).replace(r#"'"#, r#"\'"#)
}

fn filter_to_expression(filter: &BTreeMap<String, Value>) -> Result<String, Error> {
    let vals: Result<Vec<String>, Error> = filter
        .iter()
        .filter_map(|(k, v)| {
            let v = match v {
                Value::Null => return None,
                Value::Bool(_) => String::from("true"),
                Value::Number(n) => n.to_string(),
                Value::String(s) => format!("'{}'", escape_fhirpath_string(s)),
                Value::Array(_) | Value::Object(_) => {
                    return Some(Err(Error::TooComplexFilter {
                        filter: filter.to_owned(),
                    }));
                }
            };
            Some(Ok(format!("{k}={}", v)))
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

#[cfg(test)]
mod tests {
    use std::panic;

    use serde_json::Value;
    use serde_json::json;

    use crate::attribute::aidbox::Attribute;
    use crate::attribute::aidbox::Reference;
    use crate::search_param::fhir;
    use crate::search_param::{SearchParameterExpression, SearchParameterExpressionItem};

    fn create_attribute(
        resource_id: &str,
        path: Vec<&str>,
        extension_url: Option<&str>,
        r#type: Option<&str>,
    ) -> Attribute {
        Attribute {
            id: None,
            path: path.iter().map(|s| s.to_string()).collect(),
            module: None,
            text: None,
            description: None,
            resource: Reference {
                id: resource_id.to_string(),
                resource_type: "Entity".to_string(),
            },
            r#type: r#type.map(|t| Reference {
                id: t.to_string(),
                resource_type: "Entity".to_string(),
            }),
            extension_url: extension_url.map(|s| s.to_string()),
            schema: None,
            is_required: None,
            is_collection: None,
            is_open: None,
            union: None,
            is_unique: None,
            r#enum: None,
            order: None,
            is_summary: None,
            is_modifier: None,
            value_set: None,
            refers: None,
            resource_type: None,
            source: None,
        }
    }

    #[test]
    fn test_convert_path_simple() {
        let resource_type = "Patient".to_string();
        let attributes = vec![];
        let expr = expression(json!(["name", "given"]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(result, "Patient.name.given");
    }

    #[test]
    fn test_convert_path_with_index() {
        let resource_type = "Patient".to_string();
        let attributes = vec![];
        let expr = expression(json!(["name", 0, "given"]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(result, "Patient.name[0].given");
    }

    fn expression(value: Value) -> SearchParameterExpression {
        let Value::Array(values) = value else {
            panic!("Expected array value")
        };

        values
            .into_iter()
            .map(|value| match value {
                Value::Number(n) => {
                    SearchParameterExpressionItem::Index(n.as_u64().unwrap() as usize)
                }
                Value::String(s) => SearchParameterExpressionItem::Path(s),
                Value::Object(map) => {
                    SearchParameterExpressionItem::Filter(map.into_iter().collect())
                }
                _ => panic!("Unsupported value type"),
            })
            .collect()
    }

    #[test]
    fn test_convert_path_with_filter() {
        let resource_type = "Patient".to_string();
        let attributes = vec![];

        let expr = expression(json!(["name", {"use": "official"}, "given"]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(result, "Patient.name.where(use='official').given");
    }

    #[test]
    fn test_convert_path_with_attributes() {
        let resource_type = "Patient".to_string();
        let attributes = vec![
            create_attribute("Patient", vec!["name"], None, None),
            create_attribute("Patient", vec!["name", "given"], None, None),
        ];

        let expr = expression(json!(["name", "given"]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(result, "Patient.name.given");
    }

    #[test]
    fn test_convert_path_with_extension() {
        let resource_type = "Patient".to_string();
        let attributes = vec![create_attribute(
            "Patient",
            vec!["extension"],
            Some("http://example.org/fhir/StructureDefinition/custom-extension"),
            Some("string"),
        )];

        let expr = expression(json!(["extension"]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(
            result,
            "Patient.extension('http://example.org/fhir/StructureDefinition/custom-extension').value.ofType(string)"
        );
    }

    #[test]
    fn test_convert_path_with_multiple_filters() {
        let resource_type = "Patient".to_string();
        let attributes = vec![];
        let expr = expression(json!([
            "name",
            {"use": "official"},
            "telecom",
            {"system": "phone", "active": true}
        ]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(
            result,
            "Patient.name.where(use='official').telecom.where(active=true and system='phone')"
        );
    }

    #[test]
    fn test_escape_fhirpath_string() {
        let resource_type = "Patient".to_string();
        let attributes = vec![];

        let expr = expression(json!([
            "extension",
            {"url": r#"http://example.org/fhir/StructureDefinition/with'quote"andDoubleQuote\andBackSlash"#}
        ]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(
            result,
            r#"Patient.extension.where(url='http://example.org/fhir/StructureDefinition/with\'quote"andDoubleQuote\\andBackSlash')"#
        );
    }

    #[test]
    fn test_convert_path_complex_expression() {
        let resource_type = "Observation".to_string();
        let attributes = vec![
            create_attribute("Observation", vec!["code"], None, None),
            create_attribute("Observation", vec!["code", "coding"], None, None),
            create_attribute(
                "Observation",
                vec!["code", "coding", "system"],
                None,
                Some("uri"),
            ),
        ];

        let expr = expression(json!([
            "code",
            "coding",
            {"system": "http://loinc.org"},
            "code"
        ]));

        let result = fhir::convert_path(resource_type, &attributes, &expr).unwrap();
        assert_eq!(
            result,
            "Observation.code.coding.where(system='http://loinc.org').code"
        );
    }

    #[test]
    fn test_complex_filter_error() {
        let resource_type = "Patient".to_string();
        let attributes = vec![];

        let expr = expression(json!([
            "name",
            {"complex": {"key": "value"}}
        ]));

        let result = fhir::convert_path(resource_type, &attributes, &expr);
        assert!(result.is_err());
    }
}
