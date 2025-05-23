use std::collections::BTreeMap;

use crate::{attribute::typed::AttributeKind, trie::raw};

pub struct Forest {
    pub forest: BTreeMap<String, Trie>,
}

#[derive(Debug, Clone)]
pub struct Trie {
    pub root: Node,
}

#[derive(Debug, Clone)]
pub enum Node {
    Normal(NormalNode),
    Extension(Extension),
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
    pub children: BTreeMap<String, Node>,
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
    pub children: BTreeMap<String, Node>,
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
    pub children: BTreeMap<String, Node>,
    pub id: String,
    pub path: Vec<String>,
    pub required: bool,
    pub resource_type: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolymorphicExtension {
    pub array: bool,
    pub children: BTreeMap<String, Node>,
    pub fce: String,
    pub id: String,
    pub path: Vec<String>,
    pub required: bool,
    pub resource_type: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexNode {
    pub array: bool,
    pub id: String,
    pub open: bool,
    pub required: bool,
    pub resource_type: String,
    pub children: BTreeMap<String, Node>,
}

#[derive(Debug, Clone)]
pub struct ComplexExtension {
    pub array: bool,
    pub fce: String,
    pub id: String,
    pub open: bool,
    pub required: bool,
    pub resource_type: String,
    pub children: BTreeMap<String, Node>,
}

#[derive(Debug, Clone)]
pub struct InferredNode {
    pub children: BTreeMap<String, Node>,
}

impl Forest {
    pub fn new() -> Self {
        Self {
            forest: BTreeMap::new(),
        }
    }

    pub fn build_from(source_forest: &raw::Forest) -> Self {
        let mut forest = Self::new();

        for (resource_type, trie) in &source_forest.forest {
            let trie = Trie::build_from(trie);
            forest.forest.insert(resource_type.to_owned(), trie);
        }

        forest
    }
}

impl Trie {
    pub fn build_from(source_trie: &raw::Trie) -> Self {
        let root = Node::build_from(&source_trie.root);
        let trie = Self { root: root };
        trie
    }
}

impl Node {
    pub fn build_from(source_node: &raw::Node) -> Self {
        let children: BTreeMap<String, Node> = source_node
            .children
            .iter()
            .map(|(name, child)| (name.to_owned(), Self::build_from(child)))
            .collect();

        match &source_node.attribute {
            Some(attribute) => match (&attribute.kind, &attribute.fce) {
                (AttributeKind::Poly(attribute_kind_poly), None) => {
                    Node::Normal(NormalNode::Polymorphic(PolymorphicNode {
                        array: attribute.array,
                        children: children,
                        id: attribute.id.to_owned(),
                        path: attribute.path.to_owned(),
                        required: attribute.required,
                        resource_type: attribute.resource_type.to_owned(),
                        targets: attribute_kind_poly.targets.to_owned(),
                    }))
                }

                (AttributeKind::Poly(attribute_kind_poly), Some(fce)) => {
                    Node::Extension(Extension::Polymorphic(PolymorphicExtension {
                        array: attribute.array,
                        children: children,
                        id: attribute.id.to_owned(),
                        path: attribute.path.to_owned(),
                        required: attribute.required,
                        resource_type: attribute.resource_type.to_owned(),
                        targets: attribute_kind_poly.targets.to_owned(),
                        fce: fce.to_owned(),
                    }))
                }

                (AttributeKind::Concrete(attribute_kind_concrete), None) => {
                    Node::Normal(NormalNode::Concrete(ConcreteNode {
                        array: attribute.array,
                        children: children,
                        id: attribute.id.to_owned(),
                        refers: attribute_kind_concrete.refers.to_owned(),
                        required: attribute.required,
                        resource_type: attribute.resource_type.to_owned(),
                        target: attribute_kind_concrete.target.to_owned(),
                        value_set: attribute_kind_concrete.value_set.to_owned(),
                    }))
                }

                (AttributeKind::Concrete(attribute_kind_concrete), Some(fce)) => {
                    Node::Extension(Extension::Concrete(ConcreteExtension {
                        array: attribute.array,
                        children: children,
                        id: attribute.id.to_owned(),
                        refers: attribute_kind_concrete.refers.to_owned(),
                        required: attribute.required,
                        resource_type: attribute.resource_type.to_owned(),
                        target: attribute_kind_concrete.target.to_owned(),
                        value_set: attribute_kind_concrete.value_set.to_owned(),
                        fce: fce.to_owned(),
                    }))
                }

                (AttributeKind::Complex(attribute_kind_complex), None) => {
                    Node::Normal(NormalNode::Complex(ComplexNode {
                        array: attribute.array,
                        id: attribute.id.to_owned(),
                        open: attribute_kind_complex.open,
                        required: attribute.required,
                        resource_type: attribute.resource_type.to_owned(),
                        children: children,
                    }))
                }
                (AttributeKind::Complex(attribute_kind_complex), Some(fce)) => {
                    Node::Extension(Extension::Complex(ComplexExtension {
                        array: attribute.array,
                        id: attribute.id.to_owned(),
                        open: attribute_kind_complex.open,
                        required: attribute.required,
                        resource_type: attribute.resource_type.to_owned(),
                        children: children,
                        fce: fce.to_owned(),
                    }))
                }
            },
            None => Node::Normal(NormalNode::Inferred(InferredNode { children: children })),
        }
    }
}

impl Extension {
    pub fn convert_to_normal_node(&self) -> NormalNode {
        match &self {
            Extension::Concrete(concrete_extension) => NormalNode::Concrete(ConcreteNode {
                array: concrete_extension.array,
                children: concrete_extension.children.to_owned(),
                id: concrete_extension.id.to_owned(),
                refers: concrete_extension.refers.to_owned(),
                required: concrete_extension.required,
                resource_type: concrete_extension.resource_type.to_owned(),
                target: concrete_extension.target.to_owned(),
                value_set: concrete_extension.value_set.to_owned(),
            }),
            Extension::Polymorphic(polymorphic_extension) => {
                NormalNode::Polymorphic(PolymorphicNode {
                    array: polymorphic_extension.array,
                    children: polymorphic_extension.children.to_owned(),
                    id: polymorphic_extension.id.to_owned(),
                    path: polymorphic_extension.path.to_owned(),
                    required: polymorphic_extension.required,
                    resource_type: polymorphic_extension.resource_type.to_owned(),
                    targets: polymorphic_extension.targets.to_owned(),
                })
            }
            Extension::Complex(complex_extension) => NormalNode::Complex(ComplexNode {
                array: complex_extension.array,
                id: complex_extension.id.to_owned(),
                open: complex_extension.open,
                required: complex_extension.required,
                resource_type: complex_extension.resource_type.to_owned(),
                children: complex_extension.children.to_owned(),
            }),
        }
    }
}
