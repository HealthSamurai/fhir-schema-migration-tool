use std::collections::{BTreeMap, HashSet};

use thiserror::Error;

use crate::trie::extension_separated;

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
pub struct ConcreteNode {
    pub array: bool,
    pub id: String,
    pub refers: Option<Vec<String>>,
    pub required: bool,
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
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolymorphicLeaf {
    pub id: String,
    pub refers: Option<Vec<String>>,
    pub target: String,
    pub value_set: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexNode {
    pub array: bool,
    pub id: String,
    pub open: bool,
    pub required: bool,
    pub children: BTreeMap<String, NormalNode>,
    pub extension: BTreeMap<String, Extension>,
}

#[derive(Debug, Clone)]
pub struct InferredNode {
    pub children: BTreeMap<String, NormalNode>,
    pub extension: BTreeMap<String, Extension>,
}

#[derive(Debug, Clone)]
pub enum Extension {
    Simple(SimpleExtension),
    Complex(ComplexExtension),
}

#[derive(Debug, Clone)]
pub struct SimpleExtension {
    pub array: bool,
    pub targets: BTreeMap<String, ExtensionTarget>,
    pub fce_property: String,
    pub id: String,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct ExtensionTarget {
    pub id: String,
    pub refers: Option<Vec<String>>,
    pub value_set: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexExtension {
    pub array: bool,
    pub fce_property: String,
    pub id: String,
    pub open: bool,
    pub required: bool,
    pub extension: BTreeMap<String, Extension>,
}

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Polymorphic has undeclared target")]
    PolymorphicUndeclaredTarget { attr_id: String, target: String },

    #[error("Duplicate extension url {url}")]
    DuplicateExtensionUrl { url: String },
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

    pub fn build_from(source_forest: extension_separated::Forest) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut forest = Self::new();

        for (resource_type, trie) in source_forest.forest {
            let (trie, mut build_errors) = Trie::build_from(trie);
            errors.append(&mut build_errors);
            forest.forest.insert(resource_type.to_owned(), trie);
        }

        (forest, errors)
    }
}

impl Trie {
    pub fn build_from(source_trie: extension_separated::Trie) -> (Self, Vec<Error>) {
        let (root, errors) = NormalNode::build_from(source_trie.root);
        let trie = Self { root };
        (trie, errors)
    }
}

impl NormalNode {
    pub fn build_from(source_node: extension_separated::NormalNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        match source_node {
            extension_separated::NormalNode::Concrete(concrete_node) => {
                let node = ConcreteNode::build_from(concrete_node);
                (NormalNode::Concrete(node), Vec::new())
            }
            extension_separated::NormalNode::Polymorphic(polymorphic_node) => {
                let node = PolymorphicNode::build_from(polymorphic_node);
                (NormalNode::Polymorphic(node), Vec::new())
            }
            extension_separated::NormalNode::Complex(complex_node) => {
                let (node, mut build_errors) = ComplexNode::build_from(complex_node);
                errors.append(&mut build_errors);
                (NormalNode::Complex(node), errors)
            }
            extension_separated::NormalNode::Inferred(inferred_node) => {
                let (node, mut build_errors) = InferredNode::build_from(inferred_node);
                errors.append(&mut build_errors);
                (NormalNode::Inferred(node), errors)
            }
        }
    }
}

impl ConcreteNode {
    pub fn build_from(source_node: extension_separated::ConcreteNode) -> Self {
        Self {
            array: source_node.array,
            id: source_node.id,
            refers: source_node.refers,
            required: source_node.required,
            target: source_node.target,
            value_set: source_node.value_set,
        }
    }
}

impl PolymorphicLeaf {
    pub fn build_from(source_node: extension_separated::PolymorphicLeaf) -> Self {
        Self {
            id: source_node.id,
            refers: source_node.refers,
            target: source_node.target,
            value_set: source_node.value_set,
        }
    }
}

impl PolymorphicNode {
    pub fn build_from(source_node: extension_separated::PolymorphicNode) -> Self {
        let children: BTreeMap<String, PolymorphicLeaf> = source_node
            .children
            .into_iter()
            .map(|(name, child)| (name, PolymorphicLeaf::build_from(child)))
            .collect();

        Self {
            array: source_node.array,
            children,
            id: source_node.id,
            path: source_node.path,
            required: source_node.required,
            targets: source_node.targets,
        }
    }
}

impl ComplexNode {
    pub fn build_from(source_node: extension_separated::ComplexNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut children: BTreeMap<String, NormalNode> = BTreeMap::new();
        let mut extension: BTreeMap<String, Extension> = BTreeMap::new();
        for (name, source_child) in source_node.children {
            let (node, mut build_errors) = NormalNode::build_from(source_child);
            errors.append(&mut build_errors);
            children.insert(name, node);
        }

        for (name, source_ext) in source_node.extension {
            let url = source_ext.get_url().to_owned();
            let (node, mut build_errors) = Extension::build_from(source_ext, name);
            errors.append(&mut build_errors);
            if extension.contains_key(&url) {
                errors.push(Error::DuplicateExtensionUrl { url: url.clone() })
            } else {
                extension.insert(url, node);
            }
        }

        let node = Self {
            array: source_node.array,
            id: source_node.id,
            open: source_node.open,
            required: source_node.required,
            children,
            extension,
        };

        (node, errors)
    }
}

impl InferredNode {
    pub fn build_from(source_node: extension_separated::InferredNode) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut children: BTreeMap<String, NormalNode> = BTreeMap::new();
        let mut extension: BTreeMap<String, Extension> = BTreeMap::new();
        for (name, source_child) in source_node.children {
            let (node, mut build_errors) = NormalNode::build_from(source_child);
            errors.append(&mut build_errors);
            children.insert(name, node);
        }

        for (name, source_ext) in source_node.extension {
            let url = source_ext.get_url().to_owned();
            let (node, mut build_errors) = Extension::build_from(source_ext, name);
            errors.append(&mut build_errors);
            if extension.contains_key(&url) {
                errors.push(Error::DuplicateExtensionUrl { url })
            } else {
                extension.insert(url, node);
            }
        }

        let node = Self {
            children,
            extension,
        };

        (node, errors)
    }
}

impl Extension {
    pub fn build_from(
        source_node: extension_separated::Extension,
        fce_property: String,
    ) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        match source_node {
            extension_separated::Extension::Concrete(concrete_node) => {
                let node = SimpleExtension::build_from_concrete(concrete_node, fce_property);
                (Extension::Simple(node), errors)
            }
            extension_separated::Extension::Polymorphic(polymorphic_node) => {
                let (node, mut build_errors) =
                    SimpleExtension::build_from_polymorphic(polymorphic_node, fce_property);
                errors.append(&mut build_errors);
                (Extension::Simple(node), errors)
            }
            extension_separated::Extension::Complex(complex_node) => {
                let (node, mut build_errors) =
                    ComplexExtension::build_from(complex_node, fce_property);
                errors.append(&mut build_errors);
                (Extension::Complex(node), errors)
            }
        }
    }
}

impl SimpleExtension {
    pub fn build_from_concrete(
        source_node: extension_separated::ConcreteExtension,
        fce_property: String,
    ) -> Self {
        Self {
            array: source_node.array,
            targets: BTreeMap::from([(
                source_node.target,
                ExtensionTarget {
                    id: source_node.id.clone(),
                    refers: source_node.refers,
                    value_set: source_node.value_set,
                },
            )]),
            fce_property: fce_property,
            id: source_node.id,
            required: source_node.required,
        }
    }

    pub fn build_from_polymorphic(
        source_node: extension_separated::PolymorphicExtension,
        fce_property: String,
    ) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut targets: BTreeMap<String, ExtensionTarget> = BTreeMap::new();
        let declared_targets: HashSet<String> =
            HashSet::from_iter(source_node.targets.iter().cloned());
        for (name, target) in source_node.children {
            if !declared_targets.contains(&name) {
                errors.push(Error::PolymorphicUndeclaredTarget {
                    attr_id: source_node.id.clone(),
                    target: name.clone(),
                })
            };
            let target = ExtensionTarget {
                id: target.id,
                refers: target.refers,
                value_set: target.value_set,
            };
            targets.insert(name, target);
        }

        let node = Self {
            array: source_node.array,
            targets,
            fce_property: fce_property,
            id: source_node.id,
            required: source_node.required,
        };

        (node, errors)
    }
}

impl ComplexExtension {
    pub fn build_from(
        source_node: extension_separated::ComplexExtension,
        fce_property: String,
    ) -> (Self, Vec<Error>) {
        let mut errors: Vec<Error> = Vec::new();
        let mut extension: BTreeMap<String, Extension> = BTreeMap::new();

        for (name, source_ext) in source_node.extension {
            let url = source_ext.get_url().to_owned();
            let (node, mut build_errors) = Extension::build_from(source_ext, name);
            errors.append(&mut build_errors);
            if extension.contains_key(&url) {
                errors.push(Error::DuplicateExtensionUrl { url })
            } else {
                extension.insert(url, node);
            }
        }

        let node = Self {
            array: source_node.array,
            fce_property: fce_property,
            id: source_node.id,
            open: source_node.open,
            required: source_node.required,
            extension,
        };

        (node, errors)
    }
}
