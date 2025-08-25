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
    pub enumeration: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AttributeKindComplex {
    pub open: bool,
}

#[derive(Debug, Error, Diagnostic)]
#[error("Attribute {id} is invalid")]
pub struct Error {
    pub id: String,
    #[source]
    #[diagnostic_source]
    #[diagnostic(transparent)]
    pub source: InvalidAttributeError,
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

    #[error("Invalid concrete attribute.")]
    InvalidConcrete(#[from] InvalidConcrete),

    #[error("Invalid polymorphic attribute.")]
    InvalidPolymorphic(#[from] InvalidPolymorphic),

    #[error("Invalid complex attribute.")]
    InvalidComplex(#[from] InvalidComplex),
}

#[derive(Debug, Error, Diagnostic)]
pub enum InvalidPolymorphic {
    #[error("ValueSet binding on polymorphic root is not allowed")]
    #[diagnostic(help(
        "{}\n{}",
        "It is allowed by FHIR spec, but Aidbox Attribute validator doesn't support it, so the converter rejects such cases.",
        "Consider removing binding or moving it to polymorphic targets."
    ))]
    ValueSetPresent,

    #[error("isOpen on polymorhic is not allowed")]
    #[diagnostic(help(
        "It is not clear how to map isOpen to correct FHIR extensions. Contact us to come up with solution."
    ))]
    OpenSchema,

    #[error("enum on polymorphic is not allowed")]
    #[diagnostic(help(
        "{} {}",
        "Aidbox attribute validator doesn't interpret enum on polymorphic root attribute.",
        "To avoid ambiguites the converter considers it an error."
    ))]
    EnumPresent,

    #[error("Reference target binding on polymorhpic is not allowed")]
    #[diagnostic(help(
        "Reference target should be placed on concrete polymorphic choice attribute."
    ))]
    RefersPresent,

    #[error("Empty list of targets")]
    #[diagnostic(help(
        "Polymorphic element without any targets could not be present in a resource."
    ))]
    NoTargets,
}

#[derive(Debug, Error, Diagnostic)]
pub enum InvalidConcrete {
    #[error("ValueSet binding declared on type not supporting bindings: {0}")]
    #[diagnostic(help(
        "ValueSet binding can be only on coded types. Refer to the FHIR specification to get a list of all coded data types."
    ))]
    ValueSetOnWrongType(String),

    #[error("Reference target binding on non-reference type: {0}")]
    RefersOnNonReferenceType(String),

    #[error("enum specified on non-string-type: {0}")]
    EnumOnNonStirngType(String),

    #[error("isOpen is not allowed on concrete Attribute resources")]
    OpenSchema,
}

#[derive(Debug, Clone, Error)]
pub enum InvalidComplex {
    #[error("ValueSet binding is not allowed on complex attributes")]
    ValueSetPresent,

    #[error("enum is not allowed on complex attributes")]
    EnumPresent,

    #[error("refers is not allowed on complex attributes")]
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

    fn parse_resource_type(
        target: &aidbox::Reference,
    ) -> (Option<String>, Option<InvalidAttributeError>) {
        if target.resource_type != "Entity" {
            return (
                Some(target.id.to_owned()),
                Some(InvalidAttributeError::InvalidEntityReference(
                    target.to_owned(),
                )),
            );
        }

        (Some(target.id.to_owned()), None)
    }

    fn parse_type(target: &aidbox::Reference) -> (Option<String>, Option<InvalidAttributeError>) {
        if target.resource_type == "Attribute" {
            return (
                None,
                Some(InvalidAttributeError::InvalidEntityReference(
                    target.to_owned(),
                )),
            );
        }

        if target.resource_type != "Entity" {
            return (
                Some(target.id.to_owned()),
                Some(InvalidAttributeError::InvalidEntityReference(
                    target.to_owned(),
                )),
            );
        }

        (Some(target.id.to_owned()), None)
    }

    fn parse_value_set(value_set: &aidbox::Reference) -> (String, Option<InvalidAttributeError>) {
        let error = if value_set.resource_type != "ValueSet" {
            Some(InvalidAttributeError::InvalidValuesetReference(
                value_set.to_owned(),
            ))
        } else {
            None
        };
        (value_set.id.to_owned(), error)
    }

    pub fn read_target_attribute(
        attr: aidbox::Attribute,
    ) -> (Option<Attribute>, Vec<InvalidAttributeError>) {
        assert!(attr.r#type.is_some());
        assert!(attr.union.is_none());

        let mut errors: Vec<InvalidAttributeError> = Vec::new();

        // Already checked that not None
        let attr_type = attr.r#type.as_ref().unwrap();

        let (resource_type, rt_error) = Self::parse_resource_type(&attr.resource);
        if let Some(rt_error) = rt_error {
            errors.push(rt_error);
        }

        if attr.is_open.is_some_and(|x| x) {
            errors.push(InvalidConcrete::OpenSchema.into());
        }

        let value_set = if let Some(value_set_ref) = &attr.value_set {
            let (value_set, error) = Self::parse_value_set(value_set_ref);
            if let Some(error) = error {
                errors.push(error);
            }
            Some(value_set)
        } else {
            None
        };
        let value_set = value_set;

        let (target, error) = Self::parse_type(attr_type);
        if let Some(error) = error {
            errors.push(error);
        }
        if let Some(target) = target {
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
                enumeration: attr.r#enum,
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
        } else {
            (None, errors)
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

        let (resource_type, error) = Self::parse_resource_type(&attr.resource);
        if let Some(error) = error {
            errors.push(error);
        }

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
            let (target, error) = Self::parse_type(target_ref);
            if let Some(error) = error {
                errors.push(error);
            }
            if let Some(target) = target {
                targets.push(target);
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

        let (resource_type, error) = Self::parse_resource_type(&attr.resource);
        if let Some(error) = error {
            errors.push(error);
        }

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
