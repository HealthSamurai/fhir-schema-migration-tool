use std::collections::BTreeMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::trie::path;

pub struct Forest {
    pub forest: BTreeMap<String, Trie>,
}

#[derive(Debug, Clone)]
pub struct Trie {
    pub root: NormalNode,
}

#[derive(Debug, Clone)]
pub enum NormalNode {
    Concrete(ConcreteNode),
    Polymorphic(PolymorphicNode),
    Complex(ComplexNode),
    Inferred(InferredNode),
}

#[derive(Debug, Clone)]
pub enum Extension {
    Concrete(ConcreteExtension),
    Polymorphic(PolymorphicExtension),
    Complex(ComplexExtension),
}

#[derive(Debug, Clone)]
pub struct ConcreteNode {
    pub array: bool,
    pub id: String,
    pub refers: Option<Vec<String>>,
    pub required: bool,
    pub resource_type: String,
    pub target: String,
    pub value_set: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConcreteExtension {
    pub array: bool,
    pub fce: String,
    pub id: String,
    pub refers: Option<Vec<String>>,
    pub required: bool,
    pub resource_type: String,
    pub target: String,
    pub value_set: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PolymorphicNode {
    pub array: bool,
    pub children: BTreeMap<String, PolymorphicLeaf>,
    pub id: String,
    pub path: Vec<String>,
    pub required: bool,
    pub resource_type: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolymorphicExtension {
    pub array: bool,
    pub children: BTreeMap<String, PolymorphicLeaf>,
    pub fce: String,
    pub id: String,
    pub path: Vec<String>,
    pub required: bool,
    pub resource_type: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolymorphicLeaf {
    pub id: String,
    pub refers: Option<Vec<String>>,
    pub resource_type: String,
    pub target: String,
    pub value_set: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexNode {
    pub array: bool,
    pub id: String,
    pub open: bool,
    pub required: bool,
    pub resource_type: String,
    pub children: BTreeMap<String, NormalNode>,
    pub extension: BTreeMap<String, Extension>,
}

#[derive(Debug, Clone)]
pub struct ComplexExtension {
    pub array: bool,
    pub fce: String,
    pub id: String,
    pub open: bool,
    pub required: bool,
    pub resource_type: String,
    pub extension: BTreeMap<String, Extension>,
}

#[derive(Debug, Clone)]
pub struct InferredNode {
    pub children: BTreeMap<String, NormalNode>,
    pub extension: BTreeMap<String, Extension>,
}

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error(
        "Attribute {node_id} defines a concrete element. Concrete elements must not have children, but this element has."
    )]
    ConcreteHasChild { node_id: String },

    #[error(
        "Attribute {attr_id} defines a polymorphic elements. It has child {child_id} with extensionUrl set. Children of polymorphic elements must not have extensionUrl."
    )]
    #[diagnostic(help(
        "This leads to invalid conversion Aidbox->FHIR format. Aidbox->FHIR converter represents this situation as valueExtension field, which is impossible in FHIR."
    ))]
    PolymorphicChildExtension { attr_id: String, child_id: String },

    #[error(
        "Attribute {attr_id} defines a polymorphic element. It has child {child_id} which is not a concrete element (i.e. does not have type set). Every child of a polymorphic must be a concrete element."
    )]
    PolymorphicNonConcreteChild { attr_id: String, child_id: String },

    #[error(
        "Attribute {attr_id} defines a polymorphic element. It has an inferred complex child under {child_prop} property. Polymorphic elements must only have concrete, explicity children."
    )]
    PolymorphicInferredChild { attr_id: String, child_prop: String },

    #[error(
        "Attribute {attr_id} is a root attribute (empty path) and it has extensionUrl set. Root cannot be an extension."
    )]
    RootIsExtension { attr_id: String },

    #[error(
        "Attribute {parent_id} defines an extension. Its children must be extensions, but child {child_id} is not an extension."
    )]
    #[diagnostic(help("Consider assigning extensionUrl to the {child_id} attribute."))]
    NonExtensionInsideExtension { parent_id: String, child_id: String },

    #[error(
        "{} {}",
        "Attribute {parent_id} defines an extension.",
        format!("Its children must be explicitly specified, but child {child_property} has no corresponding attribute.")
    )]
    MissingChild {
        parent_id: String,
        child_property: String,
    },

    #[error(
        "Attribute {attr_id} is a child of a polymorphic Attribute. Such attributes must not set isArray (it is controlled at the polymorphic root level)."
    )]
    PolymorphicChildHasArray { attr_id: String },

    #[error(
        "Attribute {attr_id} is a child of a polymorphic Attribute. Such attributes must not set isRequired (it is controlled at the polymorphic root level)."
    )]
    PolymorphicChildIsRequired { attr_id: String },
}

impl Default for Forest {
    fn default() -> Self {
        Self::new()
    }
}

impl Forest {
    pub fn new() -> Self {
        Self {
            forest: BTreeMap::new(),
        }
    }

    pub fn build_from(source_forest: path::Forest) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut forest = Self::new();

        for (resource_type, trie) in source_forest.forest {
            let (trie, mut build_errors) = Trie::build_from(trie);
            errors.append(&mut build_errors);
            forest.forest.insert(resource_type, trie);
        }

        (forest, errors)
    }
}

impl Trie {
    pub fn build_from(source_trie: path::Trie) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let (root, errors) = match source_trie.root {
            path::Node::Normal(normal_node) => NormalNode::build_from(normal_node),
            path::Node::Extension(extension) => {
                errors.push(Error::RootIsExtension {
                    attr_id: extension.get_id().to_owned(),
                });
                NormalNode::build_from(extension.convert_to_normal_node())
            }
        };
        let trie = Self { root };
        (trie, errors)
    }
}

impl NormalNode {
    pub fn build_from(source_node: path::NormalNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        match source_node {
            path::NormalNode::Concrete(concrete_node) => {
                let (node, mut build_errors) = ConcreteNode::build_from(concrete_node);
                errors.append(&mut build_errors);
                (NormalNode::Concrete(node), errors)
            }
            path::NormalNode::Polymorphic(polymorphic_node) => {
                let (node, mut build_errors) = PolymorphicNode::build_from(polymorphic_node);
                errors.append(&mut build_errors);
                (NormalNode::Polymorphic(node), errors)
            }
            path::NormalNode::Complex(complex_node) => {
                let (node, mut build_errors) = ComplexNode::build_from(complex_node);
                errors.append(&mut build_errors);
                (NormalNode::Complex(node), errors)
            }
            path::NormalNode::Inferred(inferred_node) => {
                let (node, mut build_errors) = InferredNode::build_from(inferred_node);
                errors.append(&mut build_errors);
                (NormalNode::Inferred(node), errors)
            }
        }
    }
}

impl Extension {
    pub fn get_url(&self) -> &str {
        match &self {
            Extension::Concrete(concrete_extension) => &concrete_extension.fce,
            Extension::Polymorphic(polymorphic_extension) => &polymorphic_extension.fce,
            Extension::Complex(complex_extension) => &complex_extension.fce,
        }
    }

    pub fn build_from(source_node: path::Extension) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        match source_node {
            path::Extension::Concrete(concrete_node) => {
                let (node, mut build_errors) = ConcreteExtension::build_from(concrete_node);
                errors.append(&mut build_errors);
                (Extension::Concrete(node), errors)
            }
            path::Extension::Polymorphic(polymorphic_node) => {
                let (node, mut build_errors) = PolymorphicExtension::build_from(polymorphic_node);
                errors.append(&mut build_errors);
                (Extension::Polymorphic(node), errors)
            }
            path::Extension::Complex(complex_node) => {
                let (node, mut build_errors) = ComplexExtension::build_from(complex_node);
                errors.append(&mut build_errors);
                (Extension::Complex(node), errors)
            }
        }
    }
}

impl ConcreteNode {
    pub fn build_from(source_node: path::ConcreteNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        if !source_node.children.is_empty() {
            errors.push(Error::ConcreteHasChild {
                node_id: source_node.id.clone(),
            });
        }

        let node = Self {
            array: source_node.array,
            id: source_node.id,
            refers: source_node.refers,
            required: source_node.required,
            resource_type: source_node.resource_type,
            target: source_node.target,
            value_set: source_node.value_set,
        };

        (node, errors)
    }

    pub fn build_from_extension(source_node: path::ConcreteExtension) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        if !source_node.children.is_empty() {
            errors.push(Error::ConcreteHasChild {
                node_id: source_node.id.to_owned(),
            });
        }

        let node = Self {
            array: source_node.array,
            id: source_node.id,
            refers: source_node.refers,
            required: source_node.required,
            resource_type: source_node.resource_type,
            target: source_node.target,
            value_set: source_node.value_set,
        };

        (node, errors)
    }
}

impl ConcreteExtension {
    pub fn build_from(source_node: path::ConcreteExtension) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        if !source_node.children.is_empty() {
            errors.push(Error::ConcreteHasChild {
                node_id: source_node.id.to_owned(),
            });
        }

        let node = Self {
            array: source_node.array,
            fce: source_node.fce,
            id: source_node.id,
            refers: source_node.refers,
            required: source_node.required,
            resource_type: source_node.resource_type,
            target: source_node.target,
            value_set: source_node.value_set,
        };

        (node, errors)
    }
}

impl PolymorphicLeaf {
    pub fn build_from(source_node: path::ConcreteNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        if source_node.array {
            errors.push(Error::PolymorphicChildHasArray {
                attr_id: source_node.id.clone(),
            })
        }

        if source_node.required {
            errors.push(Error::PolymorphicChildIsRequired {
                attr_id: source_node.id.clone(),
            })
        }

        let node = Self {
            id: source_node.id,
            refers: source_node.refers,
            resource_type: source_node.resource_type,
            target: source_node.target,
            value_set: source_node.value_set,
        };

        (node, errors)
    }

    pub fn build_from_extension(source_node: path::ConcreteExtension) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        if source_node.array {
            errors.push(Error::PolymorphicChildHasArray {
                attr_id: source_node.id.clone(),
            })
        }

        if source_node.required {
            errors.push(Error::PolymorphicChildIsRequired {
                attr_id: source_node.id.clone(),
            })
        }

        let node = Self {
            id: source_node.id,
            refers: source_node.refers,
            resource_type: source_node.resource_type,
            target: source_node.target,
            value_set: source_node.value_set,
        };

        (node, errors)
    }
}

impl PolymorphicNode {
    pub fn build_from(source_node: path::PolymorphicNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut children: BTreeMap<String, PolymorphicLeaf> = BTreeMap::new();

        for (name, source_child) in source_node.children {
            match source_child {
                path::Node::Normal(path::NormalNode::Concrete(source_child)) => {
                    let (node, mut build_errors) = PolymorphicLeaf::build_from(source_child);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                path::Node::Extension(path::Extension::Concrete(source_child)) => {
                    errors.push(Error::PolymorphicChildExtension {
                        attr_id: source_node.id.clone(),
                        child_id: source_child.id.clone(),
                    });
                    let (node, mut build_errors) =
                        PolymorphicLeaf::build_from_extension(source_child);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                node => {
                    let child_id = node.get_id();
                    if let Some(child_id) = child_id {
                        errors.push(Error::PolymorphicNonConcreteChild {
                            attr_id: source_node.id.clone(),
                            child_id: child_id.to_owned(),
                        })
                    } else {
                        errors.push(Error::PolymorphicInferredChild {
                            attr_id: source_node.id.clone(),
                            child_prop: name,
                        })
                    }
                }
            };
        }

        let node = Self {
            array: source_node.array,
            children,
            id: source_node.id,
            path: source_node.path,
            required: source_node.required,
            resource_type: source_node.resource_type,
            targets: source_node.targets,
        };

        (node, errors)
    }
}

impl PolymorphicExtension {
    pub fn build_from(source_node: path::PolymorphicExtension) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut children: BTreeMap<String, PolymorphicLeaf> = BTreeMap::new();

        for (name, source_child) in source_node.children {
            match source_child {
                path::Node::Normal(path::NormalNode::Concrete(source_child)) => {
                    let (node, mut build_errors) = PolymorphicLeaf::build_from(source_child);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                path::Node::Extension(path::Extension::Concrete(source_child)) => {
                    errors.push(Error::PolymorphicChildExtension {
                        attr_id: source_node.id.clone(),
                        child_id: source_child.id.clone(),
                    });
                    let (node, mut build_errors) =
                        PolymorphicLeaf::build_from_extension(source_child);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                child => {
                    let child_id = child.get_id();
                    if let Some(child_id) = child_id {
                        errors.push(Error::PolymorphicNonConcreteChild {
                            attr_id: source_node.id.clone(),
                            child_id: child_id.to_owned(),
                        })
                    } else {
                        errors.push(Error::PolymorphicInferredChild {
                            attr_id: source_node.id.clone(),
                            child_prop: name,
                        })
                    }
                }
            };
        }

        let node = Self {
            array: source_node.array,
            children,
            id: source_node.id,
            path: source_node.path,
            required: source_node.required,
            resource_type: source_node.resource_type,
            targets: source_node.targets,
            fce: source_node.fce,
        };

        (node, errors)
    }
}

impl ComplexNode {
    pub fn build_from(source_node: path::ComplexNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut children: BTreeMap<String, NormalNode> = BTreeMap::new();
        let mut extension: BTreeMap<String, Extension> = BTreeMap::new();
        for (name, source_child) in source_node.children {
            match source_child {
                path::Node::Normal(normal_node) => {
                    let (node, mut build_errors) = NormalNode::build_from(normal_node);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                path::Node::Extension(extension_node) => {
                    let (node, mut build_errors) = Extension::build_from(extension_node);
                    errors.append(&mut build_errors);
                    extension.insert(name, node);
                }
            }
        }

        let node = Self {
            array: source_node.array,
            id: source_node.id,
            open: source_node.open,
            required: source_node.required,
            resource_type: source_node.resource_type,
            children,
            extension,
        };

        (node, errors)
    }
}

impl ComplexExtension {
    pub fn build_from(source_node: path::ComplexExtension) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut extension: BTreeMap<String, Extension> = BTreeMap::new();
        for (name, source_child) in source_node.children {
            match source_child {
                path::Node::Normal(source_child) => {
                    match source_child.get_id() {
                        Some(child_id) => {
                            errors.push(Error::NonExtensionInsideExtension {
                                parent_id: source_node.id.clone(),
                                child_id: child_id.to_owned(),
                            });
                        }
                        None => {
                            // Inferred node
                            errors.push(Error::MissingChild {
                                parent_id: source_node.id.clone(),
                                child_property: name.clone(),
                            })
                        }
                    }
                }
                path::Node::Extension(extension_node) => {
                    let (node, mut build_errors) = Extension::build_from(extension_node);
                    errors.append(&mut build_errors);
                    extension.insert(name, node);
                }
            }
        }

        let node = Self {
            array: source_node.array,
            id: source_node.id,
            open: source_node.open,
            required: source_node.required,
            resource_type: source_node.resource_type,
            extension,
            fce: source_node.fce,
        };

        (node, errors)
    }
}

impl InferredNode {
    pub fn build_from(source_node: path::InferredNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut children: BTreeMap<String, NormalNode> = BTreeMap::new();
        let mut extension: BTreeMap<String, Extension> = BTreeMap::new();
        for (name, source_child) in source_node.children {
            match source_child {
                path::Node::Normal(normal_node) => {
                    let (node, mut build_errors) = NormalNode::build_from(normal_node);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                path::Node::Extension(extension_node) => {
                    let (node, mut build_errors) = Extension::build_from(extension_node);
                    errors.append(&mut build_errors);
                    extension.insert(name, node);
                }
            }
        }

        let node = Self {
            children,
            extension,
        };

        (node, errors)
    }
}
