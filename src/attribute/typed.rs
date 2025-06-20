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

#[derive(Debug, Clone, Error)]
pub enum InvalidAttributeError {
    #[error("Both union and type cannot be present")]
    InvalidKind,

    #[error("JSON Schema is not supported")]
    SchemaPresent,

    #[error("isSummary is not supported")]
    SummaryPresent,

    #[error("isModifier is not supported")]
    ModifierPresent,

    #[error("isUnique is not supported")]
    UniquePresent,

    #[error("order is not supported")]
    OrderPresent,

    #[error("Invalid entity reference: {0:?}")]
    InvalidEntityReference(aidbox::Reference),

    #[error("Invalid ValueSet reference: {0:?}")]
    InvalidValuesetReference(aidbox::Reference),

    #[error("Invalid concrete attribute")]
    InvalidConcrete(#[from] InvalidConcrete),

    #[error("Invalid polymorphic attribute")]
    InvalidPolymorphic(#[from] InvalidPolymorphic),

    #[error("Invalid complex attribute")]
    InvalidComples(#[from] InvalidComplex),
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
    fn check_unsupported_properties(
        errors: &mut Vec<InvalidAttributeError>,
        attr: &aidbox::Attribute,
    ) {
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
        errors: &mut Vec<InvalidAttributeError>,
        attr: &aidbox::Attribute,
    ) -> Option<Attribute> {
        assert!(attr.r#type.is_some());
        assert!(attr.union.is_none());

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
                    return None;
                };

                let kind = AttributeKind::Concrete(AttributeKindConcrete {
                    target: target,
                    value_set: value_set,
                    refers: attr.refers.to_owned(),
                });
                return Some(Attribute {
                    id: attr.id.to_owned(),
                    path: attr.path.to_owned(),
                    resource_type: resource_type,
                    kind: kind,
                    array: attr.is_collection.is_some_and(|x| x),
                    required: attr.is_required.is_some_and(|x| x),
                    fce: attr.extension_url.to_owned(),
                });
            }
            Err(e) => {
                errors.push(e);
                return None;
            }
        }
    }

    fn read_poly_attribute(
        errors: &mut Vec<InvalidAttributeError>,
        attr: &aidbox::Attribute,
    ) -> Option<Attribute> {
        assert!(attr.r#type.is_none());
        assert!(attr.union.is_some());

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
            return None;
        };

        if targets.is_empty() {
            return None;
        }

        let kind = AttributeKind::Poly(AttributeKindPoly { targets: targets });
        Some(Attribute {
            id: attr.id.to_owned(),
            path: attr.path.to_owned(),
            resource_type: resource_type,
            kind: kind,
            array: attr.is_collection.is_some_and(|x| x),
            required: attr.is_required.is_some_and(|x| x),
            fce: attr.extension_url.to_owned(),
        })
    }

    fn read_complex_attribute(
        errors: &mut Vec<InvalidAttributeError>,
        attr: &aidbox::Attribute,
    ) -> Option<Attribute> {
        assert!(attr.r#type.is_none());
        assert!(attr.union.is_none());

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
            return None;
        };

        let kind = AttributeKind::Complex(AttributeKindComplex {
            open: attr.is_open.is_some_and(|x| x),
        });
        Some(Attribute {
            id: attr.id.to_owned(),
            path: attr.path.to_owned(),
            resource_type: resource_type,
            kind: kind,
            array: attr.is_collection.is_some_and(|x| x),
            required: attr.is_required.is_some_and(|x| x),
            fce: attr.extension_url.to_owned(),
        })
    }

    pub fn build_from(attr: &aidbox::Attribute) -> (Option<Self>, Vec<InvalidAttributeError>) {
        let mut errors: Vec<InvalidAttributeError> = Vec::new();

        Self::check_unsupported_properties(&mut errors, attr);

        let typed_attr = match (&attr.r#type, &attr.union) {
            (Some(_), None) => Self::read_target_attribute(&mut errors, attr),
            (None, Some(_)) => Self::read_poly_attribute(&mut errors, attr),
            (None, None) => Self::read_complex_attribute(&mut errors, attr),
            (Some(_), Some(_)) => {
                errors.push(InvalidAttributeError::InvalidKind);
                None
            }
        };

        (typed_attr, errors)
    }
}
