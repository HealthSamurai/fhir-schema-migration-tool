use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use crate::attribute::aidbox;

#[derive(Debug, Clone)]
pub struct Attribute {
    pub id: String,
    pub path: Vec<String>,
    pub resource_type: String,
    pub kind: AttributeKind,
    pub array: bool,
    pub required: bool,
    pub fce: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AttributeKind {
    /// `value[x]`
    Poly(AttributeKindPoly),
    /// property or poly target
    Concrete(AttributeKindConcrete),
    /// some.nested.properties
    Complex(AttributeKindComplex),
}

#[derive(Debug, Clone)]
pub struct AttributeKindPoly {
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AttributeKindConcrete {
    pub target: String,
    pub value_set: Option<String>,
    pub refers: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AttributeKindComplex {
    pub open: bool,
}

#[derive(Debug, Error, Diagnostic)]
#[error("Attribute {id} is invalid")]
#[diagnostic(code(E004))]
pub struct Error {
    id: String,
    #[source]
    #[diagnostic_source]
    #[diagnostic(transparent)]
    source: InvalidAttributeError,
}

#[derive(Debug, Error, Diagnostic)]
pub enum InvalidAttributeError {
    #[error("Both union and type cannot be present")]
    #[diagnostic(help(
        "In Aidbox union takes the effect. To avoid ambiguity during conversion, leave only one."
    ))]
    InvalidKind,

    #[error("schema field is present. JSON Schema is not supported")]
    #[diagnostic(help(
        "{} {}\n{}",
        "schema field is a JSON Schema for validating the property.",
        "This converter does not JSON Schema.",
        "Consider writing corresponding StructureDefinition manually."
    ))]
    SchemaPresent,

    #[error("Unsupported property: isSummary")]
    #[diagnostic(help(
        "{}\n{}",
        "isSummary makes element appear in _summary. Only FHIR itself can mark elements as summaryr.",
        "Consider removing it to conform with the FHIR spec."
    ))]
    SummaryPresent,

    #[error("Unsupported property: isModifier")]
    #[diagnostic(help(
        "{} {}\n{}",
        "isModifier marks modifier element or modifier extension.",
        "There are some additional restrictions from FHIR, so the converter does not support them.",
        "Consider removing isModifier from Attributes and adding to generated StructureDefintion resources manually."
    ))]
    ModifierPresent,

    #[error("Unsupported property: isUnique")]
    #[diagnostic(help(
        "{} {}\n{}",
        "isUnique provides automatic validation of some kind of uniqueness across all resources in database.",
        "This validation is not supported in FHIR Schema mode.",
        "Construct equivalent unique index in database and remove the isUnique on the Attribute."
    ))]
    UniquePresent,

    #[error("Unsupported property: order")]
    #[diagnostic(help(
        "{} {}\n{}",
        "The order property in Aidbox Attribute reflects the ElementDefinition position in the differential.",
        "This converter does not support order or ordered slices, and ignoring it is probably safe.",
        "But you should consider removing it."
    ))]
    OrderPresent,

    #[error("Invalid type reference resourceType: expected Entity, found {}", .0.resource_type)]
    #[diagnostic(help(
        "{} {}",
        "In valid Aidbox Attribute type is either reference to Entity, or to Attribute.",
        "Reference to Attribute is used to describe recursive structure, which is not supported by this converter",
    ))]
    InvalidEntityReference(aidbox::Reference),

    #[error("Invalid ValueSet reference resourceType: expected ValueSet, found {}", .0.resource_type)]
    #[diagnostic(help("Check ValueSet reference."))]
    InvalidValuesetReference(aidbox::Reference),

    #[error("Invalid concrete attribute")]
    InvalidConcrete(#[from] InvalidConcrete),

    #[error("Invalid polymorphic attribute")]
    InvalidPolymorphic(#[from] InvalidPolymorphic),

    #[error("Invalid complex attribute")]
    InvalidComplex(#[from] InvalidComplex),
}

#[derive(Debug, Clone, Error)]
pub enum InvalidPolymorphic {
    #[error("ValueSet binding on polymorphic is not allowed")]
    ValueSetPresent,

    #[error("isOpen on polymorhic is not allowed")]
    OpenSchema,

    #[error("enum on polymorphic is not allowed")]
    EnumPresent,

    #[error("Reference target binding on polymorhpic is not allowed")]
    RefersPresent,

    #[error("Empty list of targets")]
    NoTargets,
}

#[derive(Debug, Clone, Error)]
pub enum InvalidConcrete {
    #[error("ValueSet binding declared on type not supporting bindings: {0}")]
    ValueSetOnWrongType(String),

    #[error("Reference target binding on non-reference type: {0}")]
    RefersOnNonReferenceType(String),

    #[error("Enum specified on non-string-type: {0}")]
    EnumOnNonStirngType(String),

    #[error("isOpen is not allowed on target Attributes")]
    OpenSchema,
}

#[derive(Debug, Clone, Error)]
pub enum InvalidComplex {
    #[error("ValueSet binding is not allowed on complex attributes")]
    ValueSetPresent,

    #[error("Enum is not allowed on complex attributes")]
    EnumPresent,

    #[error("Refers is not allowed on complex attributes")]
    RefersPresent,
}

const CODED_TYPES: &[&str] = &[
    "code",
    "Coding",
    "CodeableConcept",
    "Quantity",
    "string",
    "uri",
    "Duration",
];
const STRING_TYPES: &[&str] = &[
    "base64Binary",
    "canonical",
    "code",
    "date",
    "dateTime",
    "email",
    "id",
    "instant",
    "keyword",
    "markdown",
    "oid",
    "password",
    "secret",
    "string",
    "time",
    "uri",
    "url",
    "uuid",
    "xhtml",
];

impl Attribute {
    fn check_unsupported_properties(attr: &aidbox::Attribute) -> Vec<InvalidAttributeError> {
        let mut errors: Vec<InvalidAttributeError> = Vec::new();

        if attr.schema.is_some() {
            errors.push(InvalidAttributeError::SchemaPresent);
        }

        if attr.is_summary.is_some() {
            errors.push(InvalidAttributeError::SummaryPresent);
        }

        if attr.is_modifier.is_some() {
            errors.push(InvalidAttributeError::ModifierPresent);
        }

        if attr.is_unique.is_some() {
            errors.push(InvalidAttributeError::UniquePresent);
        }

        if attr.order.is_some() {
            errors.push(InvalidAttributeError::OrderPresent);
        }

        errors
    }

    fn parse_target(target: &aidbox::Reference) -> Result<String, InvalidAttributeError> {
        if target.resource_type != "Entity" {
            return Err(InvalidAttributeError::InvalidEntityReference(
                target.to_owned(),
            ));
        }
        Ok(target.id.to_owned())
    }

    fn parse_value_set(value_set: &aidbox::Reference) -> Result<String, InvalidAttributeError> {
        if value_set.resource_type != "ValueSet" {
            return Err(InvalidAttributeError::InvalidValuesetReference(
                value_set.to_owned(),
            ));
        }
        Ok(value_set.id.to_owned())
    }

    pub fn read_target_attribute(
        attr: aidbox::Attribute,
    ) -> (Option<Attribute>, Vec<InvalidAttributeError>) {
        assert!(attr.r#type.is_some());
        assert!(attr.union.is_none());

        let mut errors: Vec<InvalidAttributeError> = Vec::new();

        // Already checked that not None
        let attr_type = attr.r#type.as_ref().unwrap();

        let resource_type = match Self::parse_target(&attr.resource) {
            Ok(rt) => Some(rt),
            Err(e) => {
                errors.push(e);
                None
            }
        };

        if attr.is_open.is_some_and(|x| x) {
            errors.push(InvalidConcrete::OpenSchema.into());
        }

        let mut value_set: Option<String> = None;
        if let Some(value_set_ref) = &attr.value_set {
            match Self::parse_value_set(value_set_ref) {
                Ok(vs) => value_set = Some(vs),
                Err(e) => errors.push(e),
            }
        };
        let value_set = value_set;

        match Self::parse_target(attr_type) {
            Ok(target) => {
                if value_set.is_some() && !CODED_TYPES.contains(&target.as_str()) {
                    errors.push(InvalidConcrete::ValueSetOnWrongType(target.clone()).into());
                }

                if attr.r#enum.is_some() && !STRING_TYPES.contains(&target.as_str()) {
                    errors.push(InvalidConcrete::EnumOnNonStirngType(target.clone()).into());
                }

                if attr.refers.is_some() && target != "Reference" {
                    errors.push(InvalidConcrete::RefersOnNonReferenceType(target.clone()).into());
                }

                let Some(resource_type) = resource_type else {
                    return (None, errors);
                };

                let kind = AttributeKind::Concrete(AttributeKindConcrete {
                    target,
                    value_set,
                    refers: attr.refers.to_owned(),
                });

                let attr = Some(Attribute {
                    id: attr.id,
                    path: attr.path,
                    resource_type,
                    kind,
                    array: attr.is_collection.is_some_and(|x| x),
                    required: attr.is_required.is_some_and(|x| x),
                    fce: attr.extension_url.to_owned(),
                });

                (attr, errors)
            }
            Err(e) => {
                errors.push(e);
                (None, errors)
            }
        }
    }

    fn read_poly_attribute(
        attr: aidbox::Attribute,
    ) -> (Option<Attribute>, Vec<InvalidAttributeError>) {
        assert!(attr.r#type.is_none());
        assert!(attr.union.is_some());

        let mut errors: Vec<InvalidAttributeError> = Vec::new();

        // Already checked that not None
        let attr_types = attr.union.as_ref().unwrap();

        let resource_type = match Self::parse_target(&attr.resource) {
            Ok(rt) => Some(rt),
            Err(e) => {
                errors.push(e);
                None
            }
        };

        if attr.is_open.is_some_and(|x| x) {
            errors.push(InvalidPolymorphic::OpenSchema.into());
        }

        if attr.value_set.is_some() {
            errors.push(InvalidPolymorphic::ValueSetPresent.into());
        }

        if attr.r#enum.is_some() {
            errors.push(InvalidPolymorphic::EnumPresent.into());
        }

        if attr.refers.is_some() {
            errors.push(InvalidPolymorphic::RefersPresent.into());
        }

        if attr_types.is_empty() {
            errors.push(InvalidPolymorphic::NoTargets.into());
        }

        let mut targets: Vec<String> = Vec::new();
        for target_ref in attr_types {
            match Self::parse_target(target_ref) {
                Ok(target) => targets.push(target),
                Err(e) => errors.push(e),
            }
        }
        let targets = targets;

        let Some(resource_type) = resource_type else {
            return (None, errors);
        };

        if targets.is_empty() {
            return (None, errors);
        }

        let kind = AttributeKind::Poly(AttributeKindPoly { targets });
        let attr = Some(Attribute {
            id: attr.id,
            path: attr.path,
            resource_type,
            kind,
            array: attr.is_collection.is_some_and(|x| x),
            required: attr.is_required.is_some_and(|x| x),
            fce: attr.extension_url,
        });

        (attr, errors)
    }

    fn read_complex_attribute(
        attr: aidbox::Attribute,
    ) -> (Option<Attribute>, Vec<InvalidAttributeError>) {
        assert!(attr.r#type.is_none());
        assert!(attr.union.is_none());

        let mut errors: Vec<InvalidAttributeError> = Vec::new();

        let resource_type = match Self::parse_target(&attr.resource) {
            Ok(rt) => Some(rt),
            Err(e) => {
                errors.push(e);
                None
            }
        };

        if attr.value_set.is_some() {
            errors.push(InvalidComplex::ValueSetPresent.into());
        }

        if attr.r#enum.is_some() {
            errors.push(InvalidComplex::EnumPresent.into());
        }

        if attr.refers.is_some() {
            errors.push(InvalidComplex::RefersPresent.into());
        }

        let Some(resource_type) = resource_type else {
            return (None, errors);
        };

        let kind = AttributeKind::Complex(AttributeKindComplex {
            open: attr.is_open.is_some_and(|x| x),
        });
        let attr = Some(Attribute {
            id: attr.id,
            path: attr.path,
            resource_type,
            kind,
            array: attr.is_collection.is_some_and(|x| x),
            required: attr.is_required.is_some_and(|x| x),
            fce: attr.extension_url,
        });
        (attr, errors)
    }

    pub fn build_from(attr: aidbox::Attribute) -> (Option<Self>, Vec<Error>) {
        let mut errors: Vec<InvalidAttributeError> = Self::check_unsupported_properties(&attr);

        let id = attr.id.clone();

        let (typed_attr, mut read_errors) = match (&attr.r#type, &attr.union) {
            (Some(_), None) => Self::read_target_attribute(attr),
            (None, Some(_)) => Self::read_poly_attribute(attr),
            (None, None) => Self::read_complex_attribute(attr),
            (Some(_), Some(_)) => (None, vec![InvalidAttributeError::InvalidKind]),
        };

        errors.append(&mut read_errors);

        let errors = errors
            .into_iter()
            .map(|error| Error {
                id: id.clone(),
                source: error,
            })
            .collect();

        (typed_attr, errors)
    }
}
