/// This is a forest (a collection of trees) of path tries of attributes
/// This is a most direct construction whic takes into the account only
/// resource type and path.
use std::collections::BTreeMap;

use thiserror::Error;

use crate::attribute::typed::Attribute;

pub struct Forest {
    pub forest: BTreeMap<String, Trie>,
}

#[derive(Debug, Clone)]
pub struct Trie {
    pub resource_type: String,
    pub root: Node,
}

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("The node at path {0} already exists")]
    AlreadyExists(String),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub attribute: Option<Attribute>,
    pub children: BTreeMap<String, Node>,
}

impl Node {
    fn new() -> Self {
        Self {
            attribute: None,
            children: BTreeMap::new(),
        }
    }
}

impl Trie {
    fn insert(&mut self, attr: Attribute) -> Result<(), Error> {
        assert_eq!(
            self.resource_type, attr.resource_type,
            "PathTrie resource type mismatch (trie type: {}; attribute type: {}",
            self.resource_type, attr.resource_type
        );
        let path = &attr.path;
        let mut node = &mut self.root;
        for path_entry in path {
            node = node
                .children
                .entry(path_entry.clone())
                .or_insert(Node::new());
        }
        if let Some(existing) = &node.attribute {
            Err(Error::AlreadyExists(existing.path.join(".")))
        } else {
            node.attribute = Some(attr);
            Ok(())
        }
    }

    fn new(resource_type: String) -> Self {
        Self {
            resource_type,
            root: Node::new(),
        }
    }
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

    pub fn insert(&mut self, attr: Attribute) -> Result<(), Error> {
        let trie = self
            .forest
            .entry(attr.resource_type.to_owned())
            .or_insert_with(|| Trie::new(attr.resource_type.to_owned()));

        trie.insert(attr)
    }

    pub fn build_from_attributes(attrs: &[Attribute]) -> (Self, Vec<Error>) {
        let mut forest = Self::new();
        let mut errors: Vec<Error> = Vec::new();
        for attr in attrs {
            match forest.insert(attr.to_owned()) {
                Ok(_) => (),
                Err(e) => errors.push(e),
            }
        }

        (forest, errors)
    }
}
