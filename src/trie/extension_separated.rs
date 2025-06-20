use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Todo")]
    Todo,

    #[error("Concrete element has child")]
    ConcreteHasChild,

    #[error("Polymorphic has child with extension")]
    PolymorphicChildExtension,

    #[error("Polymorphic has non-concrete child")]
    PolymorphicNonConcreteChild,

    #[error("Root is extension")]
    RootIsExtension,

    #[error("Non-extension inside extension")]
    NonExtensionInsideExtension,

    #[error("Polymorphic target can not set isArray")]
    PolymorphicChildHasArray,

    #[error("Polymorphic target can not set isRequired")]
    PolymorphicChildIsRequired,
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
                errors.push(Error::RootIsExtension);
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
            errors.push(Error::ConcreteHasChild);
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
            errors.push(Error::ConcreteHasChild);
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
            errors.push(Error::ConcreteHasChild);
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
            errors.push(Error::PolymorphicChildHasArray)
        }

        if source_node.required {
            errors.push(Error::PolymorphicChildIsRequired)
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
            errors.push(Error::PolymorphicChildHasArray)
        }

        if source_node.required {
            errors.push(Error::PolymorphicChildIsRequired)
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
                    errors.push(Error::PolymorphicChildExtension);
                    let (node, mut build_errors) =
                        PolymorphicLeaf::build_from_extension(source_child);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                _ => errors.push(Error::PolymorphicNonConcreteChild),
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
                    errors.push(Error::PolymorphicChildExtension);
                    let (node, mut build_errors) =
                        PolymorphicLeaf::build_from_extension(source_child);
                    errors.append(&mut build_errors);
                    children.insert(name, node);
                }
                _ => errors.push(Error::PolymorphicNonConcreteChild),
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
                path::Node::Normal(_) => {
                    errors.push(Error::NonExtensionInsideExtension);
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
