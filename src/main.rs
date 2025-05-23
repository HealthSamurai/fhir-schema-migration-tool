pub mod attribute;
pub mod paths;
pub mod trie;

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::BufReader,
    iter::{self, once},
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use walkdir::{DirEntry, WalkDir};

use crate::trie::inverted;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAttribute {
    id: String,
    path: Vec<String>,
    resource: RawReference,
    extension_url: Option<String>,
    #[serde(default)]
    is_required: bool,
    #[serde(default)]
    is_collection: bool,
    r#type: Option<RawReference>,
    union: Option<Vec<RawReference>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReference {
    id: String,
    resource_type: String,
}

#[derive(Debug, Clone)]
struct Attribute {
    id: String,
    resource_type: String,
    kind: AttributeKind,
    min: usize,
    max: String,
    path: Vec<String>,
}

#[derive(Debug, Clone)]
enum AttributeKind {
    /// `extension.value[x]`
    PolyExt { targets: Vec<String>, url: String },
    /// `value[x]`
    Poly { targets: Vec<String> },
    /// `extension.value[x]:valueString`
    SingleExt { target: String, url: String },
    /// property or poly target
    Single { target: String },
    /// some.nested.properties
    Complex {},
    /// extension:a.extension:b
    ComplexExt { url: String },
}

impl TryFrom<RawAttribute> for Attribute {
    type Error = anyhow::Error;

    fn try_from(value: RawAttribute) -> Result<Self, Self::Error> {
        let kind = match (value.extension_url, value.r#type, value.union) {
            (None, None, None) => AttributeKind::Complex {},
            (None, None, Some(targets)) => AttributeKind::Poly {
                targets: targets.into_iter().map(|target| target.id).collect(),
            },
            (None, Some(fhir_type), None) => AttributeKind::Single {
                target: fhir_type.id,
            },
            (None, Some(_), Some(_)) => {
                return Err(anyhow!("type and union can't both be present."));
            }
            (Some(url), None, None) => AttributeKind::ComplexExt { url: url },
            (Some(url), None, Some(targets)) => AttributeKind::PolyExt {
                targets: targets.into_iter().map(|target| target.id).collect(),
                url: url,
            },
            (Some(url), Some(fhir_type), None) => AttributeKind::SingleExt {
                target: fhir_type.id,
                url: url,
            },
            (Some(_), Some(_), Some(_)) => {
                return Err(anyhow!("type and union can't both be present"));
            }
        };

        let resource_type = value.resource.id;
        let path = value.path;
        let min = if value.is_required { 1 } else { 0 };
        let max = if value.is_collection {
            "*".to_owned()
        } else {
            "1".to_owned()
        };
        let id = value.id;

        Ok(Self {
            resource_type: resource_type,
            kind: kind,
            path: path,
            min: min,
            max: max,
            id: id,
        })
    }
}

#[derive(Debug, Clone)]
struct AttributeForest {
    inner: BTreeMap<String, AttributeTrie>,
}

#[derive(Debug, Clone)]
enum AttributeTrieAttribute {
    Present(Attribute),
    NotPresent,
}

#[derive(Debug, Clone)]
struct AttributeTrieInner {
    children: BTreeMap<String, AttributeTrieInner>,
    attr: AttributeTrieAttribute,
}

impl AttributeTrieInner {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            attr: AttributeTrieAttribute::NotPresent,
        }
    }
}

pub fn capitalize(s: &str) -> String {
    let mut it = s.chars();
    if let Some(first) = it.next() {
        once(first.to_ascii_uppercase()).chain(it).collect()
    } else {
        s.to_owned()
    }
}

#[derive(Debug, Clone)]
struct AttributeTrie {
    inner: AttributeTrieInner,
    rt: String,
}

impl AttributeTrie {
    pub fn new(rt: String) -> Self {
        Self {
            inner: AttributeTrieInner::new(),
            rt: rt,
        }
    }

    pub fn insert(&mut self, attribute: &Attribute) -> anyhow::Result<()> {
        let mut node = &mut self.inner;
        let path = &attribute.path;

        for component in path {
            node = node
                .children
                .entry(component.clone())
                .or_insert(AttributeTrieInner::new());
        }

        match &mut node.attr {
            AttributeTrieAttribute::Present(attribute) => return Err(anyhow!("Found duplicate")),
            AttributeTrieAttribute::NotPresent => {
                node.attr = AttributeTrieAttribute::Present(attribute.clone());
            }
        };

        Ok(())
    }

    pub fn compute_elements(&self) -> anyhow::Result<Vec<Element>> {
        fn step<'a>(
            elements: &mut Vec<Element>,
            component: &'a str,
            node: &'a AttributeTrieInner,
            state: &'a WalkState,
        ) -> anyhow::Result<WalkState<'a>> {
            use AttributeTrieAttribute::*;
            use ExtensionState::*;
            use PolyState::*;
            use PrefixState::*;

            let Present(attr) = &node.attr else {
                match state.prefix {
                    InPrefix => {
                        let state = state.child().add(component.to_owned(), None);
                        return Ok(state);
                    }
                    InAttributes => return Err(anyhow!("jump detected")),
                }
            };

            let state = state.to_owned().end_prefix();
            let new_state = match (&attr.kind, &state.poly, &state.ext) {
                (AttributeKind::Single { target }, InPoly(current_poly), InExt) => {
                    if !current_poly.targets.contains(target) {
                        return Err(anyhow!("non-declared polymorphic target"));
                    }

                    let this_slice_name = format!("value{}", capitalize(&component));
                    let mut state = state
                        .child()
                        .add("value[x]".to_owned(), Some(this_slice_name));
                    if attr.min != 0 || attr.max != "1" {
                        state = state.min(attr.min).max(attr.max.to_owned());
                        elements.push(state.generate_element());
                    }
                    state
                }
                (AttributeKind::Single { target }, _, _) => {
                    return Err(anyhow!("not allowed"));
                }
                (AttributeKind::Poly { targets: _ }, _, _) => {
                    return Err(anyhow!("raw poly not allowed"));
                }
                (_, PolyState::InPoly(_), _) => {
                    return Err(anyhow!("non-ext inside ext"));
                }
                (AttributeKind::Complex {}, _, InExt) => {
                    return Err(anyhow!("cannot be inside ext"));
                }
                (AttributeKind::PolyExt { targets, url }, PolyState::NotInPoly, _) => {
                    let this_slice_name = component.to_owned();
                    let state = state
                        .child()
                        .make_poly(CurrentPoly {
                            name: &component,
                            targets: targets,
                        })?
                        .make_ext()
                        .add("extension".to_owned(), Some(this_slice_name))
                        .min(attr.min)
                        .max(attr.max.to_owned());
                    elements.push(state.generate_element());

                    let value_state = state
                        .child()
                        .add("value[x]".to_owned(), None)
                        .set_types(targets.to_owned());
                    elements.push(value_state.generate_element());

                    state
                }
                (AttributeKind::SingleExt { target, url }, PolyState::NotInPoly, _) => {
                    let mut state = state
                        .child()
                        .make_ext()
                        .add("extension".to_owned(), Some(component.to_owned()))
                        .min(attr.min)
                        .max(attr.max.to_owned());

                    if let InExt = state.ext {
                        state = state.closed_slicing();
                    }

                    elements.push(state.generate_element());
                    elements.push(state.generate_url_match_element(url.to_owned()));
                    elements.push(
                        state
                            .child()
                            .add("value[x]".to_owned(), None)
                            .min(1)
                            .max("1".to_owned())
                            .set_types(vec![target.to_owned()])
                            .generate_element(),
                    );
                    state
                }
                (AttributeKind::Complex {}, NotInPoly, NotInExt) => {
                    elements.push(
                        state
                            .child()
                            .add(component.to_owned(), None)
                            .min(attr.min)
                            .max(attr.max.to_owned())
                            .generate_element(),
                    );
                    state
                }
                (AttributeKind::ComplexExt { url }, NotInPoly, NotInExt) => {
                    let state = state
                        .child()
                        .make_ext()
                        .add("extension".to_owned(), Some(component.to_owned()))
                        .min(attr.min)
                        .max(attr.max.to_owned());
                    elements.push(state.generate_element());
                    let extstate = state
                        .child()
                        .add("extension".to_owned(), None)
                        .closed_slicing();
                    elements.push(extstate.generate_element());
                    elements.push(state.generate_url_match_element(url.to_string()));
                    state
                }

                (AttributeKind::ComplexExt { url }, NotInPoly, InExt) => {
                    state
                        .child()
                        .make_ext()
                        .closed_slicing()
                        .min(attr.min)
                        .max(attr.max.to_owned())
                        .add("extension".to_owned(), Some(component.to_owned()));
                    elements.push(state.generate_element());
                    let extstate = state
                        .child()
                        .add("extension".to_owned(), None)
                        .closed_slicing();
                    elements.push(extstate.generate_element());
                    elements.push(state.generate_url_match_element(url.to_string()));
                    state
                }
            };
            Ok(new_state)
        }

        fn walk<'a>(
            elements: &mut Vec<Element>,
            component: &'a str,
            node: &'a AttributeTrieInner,
            state: &'a WalkState,
        ) -> anyhow::Result<()> {
            let state = step(elements, component, node, state)?;
            for (k, v) in &node.children {
                walk(elements, k, v, &state)?;
            }
            Ok(())
        }

        let mut elements: Vec<Element> = Vec::new();
        let state = WalkState::new(self.rt.to_owned());
        let node = &self.inner;
        for (k, v) in &node.children {
            walk(&mut elements, k, v, &state)?;
        }
        Ok(elements)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Element {
    id: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    slice_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slicing: Option<ElementSlicing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<Vec<ElementType>>,
}

#[derive(Debug, Clone, Serialize)]
struct ElementType {
    code: String,
}

#[derive(Debug, Clone, Serialize)]
struct ElementSlicing {
    rules: String,
}

#[derive(Debug, Clone)]
struct CurrentPoly<'a> {
    name: &'a str,
    targets: &'a Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum ExtensionState {
    InExt,
    NotInExt,
}

#[derive(Debug, Clone)]
enum PolyState<'a> {
    InPoly(CurrentPoly<'a>),
    NotInPoly,
}

#[derive(Debug, Clone, Copy)]
enum PrefixState {
    InPrefix,
    InAttributes,
}

#[derive(Debug, Clone)]
struct WalkState<'a> {
    ext: ExtensionState,
    poly: PolyState<'a>,
    prefix: PrefixState,
    id: String,
    path: String,
    slice: Option<String>,
    min: Option<usize>,
    max: Option<String>,
    slicing_closed: bool,
    types: Option<Vec<String>>,
}

impl<'a> WalkState<'a> {
    pub fn new(rt: String) -> Self {
        Self {
            ext: ExtensionState::NotInExt,
            poly: PolyState::NotInPoly,
            prefix: PrefixState::InPrefix,
            id: String::from(&rt),
            path: String::from(rt),
            slice: None,
            min: None,
            max: None,
            slicing_closed: false,
            types: None,
        }
    }

    pub fn child(&self) -> Self {
        let mut new = self.to_owned();
        new.slice = None;
        new.min = None;
        new.max = None;
        new.slicing_closed = false;
        new.types = None;
        new
    }

    pub fn min(mut self, min: usize) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: String) -> Self {
        self.max = Some(max);
        self
    }

    pub fn add(mut self, path_component: String, slice: Option<String>) -> Self {
        self.slice = None;

        self.path.push('.');
        self.path.push_str(&path_component);

        self.id.push('.');
        self.id.push_str(&path_component);
        if let Some(slice) = slice {
            self.id.push(':');
            self.id.push_str(&slice);

            self.slice = Some(slice);
        } else {
            self.slice = None
        }

        self
    }

    pub fn make_poly(mut self, poly: CurrentPoly<'a>) -> anyhow::Result<Self> {
        if let PolyState::InPoly(_) = self.poly {
            return Err(anyhow!("already polymorphic"));
        }
        self.poly = PolyState::InPoly(poly);
        Ok(self)
    }

    pub fn make_ext(mut self) -> Self {
        self.ext = ExtensionState::InExt;
        self
    }

    pub fn end_prefix(mut self) -> Self {
        self.prefix = PrefixState::InAttributes;
        self
    }

    pub fn closed_slicing(mut self) -> Self {
        self.slicing_closed = true;
        self
    }

    pub fn set_types(mut self, types: Vec<String>) -> Self {
        self.types = Some(types);
        self
    }

    pub fn generate_element(&self) -> Element {
        let slicing = if self.slicing_closed {
            Some(ElementSlicing {
                rules: "closed".to_owned(),
            })
        } else {
            None
        };

        let types = self.types.as_ref().map(|types| {
            types
                .iter()
                .map(|t| ElementType { code: t.to_owned() })
                .collect()
        });

        Element {
            id: self.id.to_owned(),
            path: self.path.to_owned(),
            slice_name: self.slice.to_owned(),
            min: self.min.to_owned(),
            max: self.max.to_owned(),
            fixed_url: None,
            slicing: slicing,
            r#type: types,
        }
    }

    pub fn generate_url_match_element(&self, url: String) -> Element {
        let mut new = self.to_owned();
        new.child().add("url".to_owned(), None);
        let mut el = new.min(1).max("1".to_owned()).generate_element();
        el.fixed_url = Some(url);
        el
    }
}

fn generate_element(element: &mut Vec<Element>, attrs: AttributeTrie) {}

fn oldmain() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let json = std::fs::read_to_string(args.get(1).with_context(|| "arg 1 must be filename")?)?;
    let attributes: Vec<RawAttribute> = serde_json::from_str(&json).unwrap();
    let attributes: Vec<Attribute> = attributes
        .into_iter()
        .map(|x| x.try_into().unwrap())
        .collect();
    let mut forest = HashMap::new();
    for attr in &attributes {
        let trie = forest
            .entry(attr.resource_type.to_owned())
            .or_insert(AttributeTrie::new(attr.resource_type.to_owned()));
        trie.insert(attr).unwrap();
    }

    for (rt, trie) in forest {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "differential": {
                    "element": &trie.compute_elements()?
                },
                "url": format!("http://URL/{}", &rt),
                "name": &rt,
                "status": "active"
            }))?
        );
    }

    Ok(())
}

/// Generate structure definition from Aidbox attributes
#[derive(Debug, Parser)]
#[command(arg_required_else_help = true)]
struct Args {
    /// path with Attribute files
    path: PathBuf,
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

fn is_json_or_yaml(path: &Path) -> bool {
    is_json(path) || is_yaml(path)
}

fn main() -> anyhow::Result<()> {
    let mut had_errors = false;
    let args = Args::parse();
    let path = args.path;

    let walker = WalkDir::new(path).into_iter();

    let mut aidbox_attributes: Vec<attribute::aidbox::Attribute> = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("{}", error);
                continue;
            }
        };

        let path = entry.path();
        if !is_json_or_yaml(path) {
            continue;
        }
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) => {
                had_errors = true;
                eprintln!("{}", error);
                continue;
            }
        };
        let file = BufReader::new(file);

        let aidbox_attribute = if is_json(path) {
            match attribute::aidbox::Attribute::from_json(file) {
                Ok(attribute) => attribute,
                Err(error) => {
                    had_errors = true;
                    eprintln!("{}", error);
                    continue;
                }
            }
        } else {
            match attribute::aidbox::Attribute::from_yaml(file) {
                Ok(attribute) => attribute,
                Err(error) => {
                    had_errors = true;
                    eprintln!("{}", error);
                    continue;
                }
            }
        };

        aidbox_attributes.push(aidbox_attribute);
    }

    let mut typed_attributes: Vec<attribute::typed::Attribute> = Vec::new();

    for aidbox_attribute in aidbox_attributes {
        match attribute::typed::Attribute::try_from(aidbox_attribute.clone()) {
            Ok(typed_attribute) => typed_attributes.push(typed_attribute),
            Err(errors) => {
                had_errors = true;
                for error in errors {
                    eprintln!("{}", error);
                }
                continue;
            }
        }
    }

    let (raw_forest, errors) = trie::raw::Forest::build_from_attributes(&typed_attributes);
    if let Some(errors) = errors {
        had_errors = true;
        for error in errors {
            eprintln!("{}", error);
        }
    }

    let path_forest = trie::path::Forest::build_from(&raw_forest);
    let (extension_separated_forest, errors) =
        trie::extension_separated::Forest::build_from(&path_forest);

    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    let (inverted_forest, errors) = trie::inverted::Forest::build_from(&extension_separated_forest);
    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    let (exts, errors) = trie::fhir::collect_extensions(&inverted_forest);

    if !errors.is_empty() {
        had_errors = true;
    }
    for error in errors {
        eprintln!("{}", error);
    }

    for ext in exts {
        println!("{}", serde_json::to_string_pretty(&ext).unwrap());
    }

    if had_errors {
        Err(anyhow!("Error :("))
    } else {
        Ok(())
    }
}
