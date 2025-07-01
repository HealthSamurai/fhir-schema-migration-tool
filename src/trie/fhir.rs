use serde::Serialize;
use thiserror::Error;

use crate::trie::inverted::{self, NormalNode};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDefinition {
    pub id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slice_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<ElementSlicing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<Vec<ElementType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<Binding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<Extension>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    url: String,
    value_string: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub value_set: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementType {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_profile: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElementSlicing {
    pub rules: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureDefinition {
    pub status: String,
    pub base_definition: String,
    pub r#abstract: bool,
    pub url: String,
    pub name: String,
    pub derivation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<StructureDefinitionContext>>,
    pub differential: StructureDefinitionDifferential,
    pub kind: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructureDefinitionContext {
    pub r#type: String,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructureDefinitionDifferential {
    pub element: Vec<ElementDefinition>,
}

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Todo")]
    Todo,
}

fn collect_extensions_recursive(
    rt: &str,
    path: &[String],
    node: inverted::NormalNode,
) -> (Vec<StructureDefinition>, Vec<Error>) {
    let mut result: Vec<StructureDefinition> = Vec::new();
    let mut errors: Vec<Error> = Vec::new();

    match node {
        inverted::NormalNode::Concrete(_) => (),
        inverted::NormalNode::Polymorphic(_) => (),
        inverted::NormalNode::Complex(complex_node) => {
            for (field, child) in complex_node.children {
                let mut child_path = path.to_owned();
                child_path.push(field.to_owned());
                let (mut child_res, mut child_errors) =
                    collect_extensions_recursive(rt, &child_path, child);
                result.append(&mut child_res);
                errors.append(&mut child_errors);
            }

            for (url, ext) in complex_node.extension {
                let ext = emit_extension(rt, path, url, ext);
                result.push(ext);
            }
        }
        inverted::NormalNode::Inferred(inferred_node) => {
            for (field, child) in inferred_node.children {
                let mut child_path = path.to_owned();
                child_path.push(field.to_owned());
                let (mut child_res, mut child_errors) =
                    collect_extensions_recursive(rt, &child_path, child);
                result.append(&mut child_res);
                errors.append(&mut child_errors);
            }
            for (url, ext) in inferred_node.extension {
                let ext = emit_extension(rt, path, url, ext);
                result.push(ext);
            }
        }
    }

    (result, errors)
}

pub fn collect_extensions(forest: inverted::Forest) -> (Vec<StructureDefinition>, Vec<Error>) {
    let mut errors: Vec<Error> = Vec::new();
    let mut sds: Vec<StructureDefinition> = Vec::new();
    for (rt, trie) in forest.forest {
        let (mut extensions, mut collect_errors) =
            collect_extensions_recursive(&rt, &[], trie.root);
        sds.append(&mut extensions);
        errors.append(&mut collect_errors);
    }
    (sds, errors)
}

pub struct ElementPointer {
    pub path: String,
    pub id: String,
}

pub fn emit_extension(
    rt: &str,
    path: &[String],
    url: String,
    extension: inverted::Extension,
) -> StructureDefinition {
    let mut base_path = "Extension".to_owned();
    for path_element in path {
        base_path.push('.');
        base_path.push_str(path_element);
    }

    let name = match &extension {
        inverted::Extension::Simple(simple_extension) => simple_extension.fce_property.to_owned(),
        inverted::Extension::Complex(complex_extension) => {
            complex_extension.fce_property.to_owned()
        }
    };

    StructureDefinition {
        base_definition: "http://hl7.org/fhir/StructureDefinition/Extension".to_owned(),
        r#abstract: false,
        status: "active".to_owned(),
        url: url.to_owned(),
        differential: StructureDefinitionDifferential {
            element: emit_differential(url, extension),
        },
        name: name,
        derivation: "constraint".to_owned(),
        context: Some(vec![StructureDefinitionContext {
            r#type: "element".to_owned(),
            expression: path.iter().fold(rt.to_owned(), |mut acc, component| {
                acc.push('.');
                acc.push_str(component);
                acc
            }),
        }]),
        kind: "complex-type".to_owned(),
        r#type: "Extension".to_owned(),
    }
}

pub fn emit_differential(url: String, extension: inverted::Extension) -> Vec<ElementDefinition> {
    match extension {
        inverted::Extension::Simple(simple_extension) => {
            let min = if simple_extension.required { 1 } else { 0 };
            let max = if simple_extension.array {
                "*".to_owned()
            } else {
                "1".to_owned()
            };
            let root = ElementDefinition {
                id: "Extension".to_owned(),
                path: "Extension".to_owned(),
                slice_name: None,
                min: Some(min),
                max: Some(max),
                fixed_url: None,
                slicing: None,
                r#type: None,
                binding: None,
                extension: Some(vec![Extension {
                    url: "http://fhir.aidbox.app/fhir/StructureDefinition/legacy-fce".to_owned(),
                    value_string: simple_extension.fce_property,
                }]),
            };

            let url_elem = ElementDefinition {
                id: "Extension.url".to_owned(),
                path: "Extension.url".to_owned(),
                slice_name: None,
                min: Some(1),
                max: Some("1".to_owned()),
                fixed_url: Some(url),
                slicing: None,
                r#type: None,
                binding: None,
                extension: None,
            };

            let value_elem = ElementDefinition {
                id: "Extension.value[x]".to_owned(),
                path: "Extension.value[x]".to_owned(),
                slice_name: None,
                min: Some(1),
                max: Some("1".to_owned()),
                fixed_url: None,
                slicing: None,
                r#type: Some(
                    simple_extension
                        .targets
                        .iter()
                        .map(|(target_type, target_info)| ElementType {
                            code: target_type.to_owned(),
                            profile: None,
                            target_profile: target_info.refers.as_ref().map(|refs| {
                                refs.into_iter()
                                    .map(|tref| format!("http://hl7.org/fhir/{}", tref))
                                    .collect()
                            }),
                        })
                        .collect(),
                ),
                binding: None,
                extension: None,
            };

            let mut differential = vec![root, url_elem, value_elem];

            for (type_name, target) in simple_extension.targets {
                if let Some(vs) = &target.value_set {
                    let elem = ElementDefinition {
                        id: format!("Extension.value[x]:value{}", type_name),
                        path: "Extension.value[x]".to_owned(),
                        slice_name: Some(format!("value{}", type_name)),
                        min: None,
                        max: None,
                        fixed_url: None,
                        slicing: None,
                        r#type: None,
                        binding: Some(Binding {
                            value_set: vs.to_owned(),
                        }),
                        extension: None,
                    };
                    differential.push(elem);
                }
            }

            differential
        }
        inverted::Extension::Complex(complex_extension) => {
            let min = if complex_extension.required { 1 } else { 0 };
            let max = if complex_extension.array {
                "*".to_owned()
            } else {
                "1".to_owned()
            };
            let root = ElementDefinition {
                id: "Extension".to_owned(),
                path: "Extension".to_owned(),
                slice_name: None,
                min: Some(min),
                max: Some(max),
                fixed_url: None,
                slicing: None,
                r#type: None,
                binding: None,
                extension: Some(vec![Extension {
                    url: "http://fhir.aidbox.app/fhir/StructureDefinition/legacy-fce".to_owned(),
                    value_string: complex_extension.fce_property,
                }]),
            };

            let base_elem = ElementDefinition {
                id: "Extension.extension".to_owned(),
                path: "Extension.extension".to_owned(),
                slice_name: None,
                min: Some(1),
                max: None,
                fixed_url: None,
                slicing: Some(ElementSlicing {
                    rules: "open".to_owned(),
                }),
                r#type: None,
                binding: None,
                extension: None,
            };

            let url_elem = ElementDefinition {
                id: "Extension.url".to_owned(),
                path: "Extension.url".to_owned(),
                slice_name: None,
                min: Some(1),
                max: Some("1".to_owned()),
                fixed_url: Some(url.to_owned()),
                slicing: None,
                r#type: None,
                binding: None,
                extension: None,
            };

            let value_elem = ElementDefinition {
                id: "Extension.value[x]".to_owned(),
                path: "Extension.value[x]".to_owned(),
                slice_name: None,
                min: Some(0),
                max: Some("0".to_owned()),
                fixed_url: None,
                slicing: None,
                r#type: None,
                binding: None,
                extension: None,
            };

            let mut nested: Vec<ElementDefinition> = Vec::new();

            let ptr = ElementPointer {
                path: "Extension.extension".to_owned(),
                id: "Extension.extension".to_owned(),
            };

            for (url, child) in complex_extension.extension {
                nested.append(&mut emit_nested(&ptr, url, child));
            }

            let mut res = Vec::new();
            // NOTE: FHIR prescribes specific order.
            res.push(root);
            res.push(base_elem);
            res.append(&mut nested);
            res.push(url_elem);
            res.push(value_elem);
            res
        }
    }
}

pub fn emit_nested(
    ptr: &ElementPointer,
    url: String,
    extension: inverted::Extension,
) -> Vec<ElementDefinition> {
    match extension {
        inverted::Extension::Simple(simple_extension) => {
            let base_elem = ElementDefinition {
                id: format!("{}:{}", ptr.id, simple_extension.fce_property),
                path: ptr.path.to_owned(),
                slice_name: Some(simple_extension.fce_property.to_owned()),
                min: Some(0),
                max: Some("*".to_owned()),
                fixed_url: None,
                slicing: None,
                r#type: None,
                binding: None,
                extension: Some(vec![Extension {
                    url: "http://fhir.aidbox.app/fhir/StructureDefinition/legacy-fce".to_owned(),
                    value_string: simple_extension.fce_property.to_owned(),
                }]),
            };

            let base_elem_ptr = ElementPointer {
                path: base_elem.path.to_owned(),
                id: base_elem.id.to_owned(),
            };

            let url_elem = ElementDefinition {
                id: format!("{}.url", base_elem_ptr.id),
                path: format!("{}.url", base_elem_ptr.path),
                slice_name: None,
                min: Some(1),
                max: Some("1".to_owned()),
                fixed_url: Some(url.to_owned()),
                slicing: None,
                r#type: None,
                binding: None,
                extension: None,
            };

            let value_elem = ElementDefinition {
                id: format!("{}.value[x]", base_elem_ptr.id),
                path: format!("{}.value[x]", base_elem_ptr.path),
                slice_name: None,
                min: Some(1),
                max: Some("1".to_owned()),
                fixed_url: None,
                slicing: None,
                r#type: Some(
                    simple_extension
                        .targets
                        .iter()
                        .map(|(target_type, target_info)| ElementType {
                            code: target_type.to_owned(),
                            profile: None,
                            target_profile: target_info.refers.as_ref().map(|refs| {
                                refs.iter()
                                    .map(|tref| format!("http://hl7.org/fhir/{}", tref))
                                    .collect()
                            }),
                        })
                        .collect(),
                ),
                binding: None,
                extension: None,
            };

            let value_elem_ptr = ElementPointer {
                path: value_elem.path.to_owned(),
                id: value_elem.id.to_owned(),
            };

            let mut differential = vec![base_elem, url_elem, value_elem];

            for (type_name, target) in simple_extension.targets {
                if let Some(vs) = &target.value_set {
                    let elem = ElementDefinition {
                        id: format!("{}:value{}", value_elem_ptr.id, type_name),
                        path: value_elem_ptr.path.to_owned(),
                        slice_name: Some(format!("value{}", type_name)),
                        min: None,
                        max: None,
                        fixed_url: None,
                        slicing: None,
                        r#type: None,
                        binding: Some(Binding {
                            value_set: vs.to_owned(),
                        }),
                        extension: None,
                    };
                    differential.push(elem);
                }
            }

            differential
        }
        inverted::Extension::Complex(complex_extension) => {
            let min = if complex_extension.required {
                Some(1)
            } else {
                None
            };
            let max = if complex_extension.array {
                None
            } else {
                Some("1".to_owned())
            };
            let base_elem = ElementDefinition {
                id: format!("{}:{}", ptr.id, complex_extension.fce_property),
                path: ptr.path.to_owned(),
                slice_name: None,
                min: min,
                max: max,
                fixed_url: None,
                slicing: None,
                r#type: None,
                binding: None,
                extension: Some(vec![Extension {
                    url: "http://fhir.aidbox.app/fhir/StructureDefinition/legacy-fce".to_owned(),
                    value_string: complex_extension.fce_property.to_owned(),
                }]),
            };

            let base_elem_ptr = ElementPointer {
                path: base_elem.path.to_owned(),
                id: base_elem.id.to_owned(),
            };

            let extension_elem = ElementDefinition {
                id: format!("{}.extension", base_elem_ptr.id),
                path: format!("{}.extension", base_elem_ptr.path),
                slice_name: None,
                min: Some(1),
                max: None,
                fixed_url: None,
                slicing: Some(ElementSlicing {
                    rules: "open".to_owned(),
                }),
                r#type: None,
                binding: None,
                extension: None,
            };

            let extension_elem_ptr = ElementPointer {
                path: extension_elem.path.to_owned(),
                id: extension_elem.id.to_owned(),
            };

            let url_elem = ElementDefinition {
                id: format!("{}.url", base_elem_ptr.id),
                path: format!("{}.url", base_elem_ptr.path),
                slice_name: None,
                min: Some(1),
                max: Some("1".to_owned()),
                fixed_url: Some(url.to_owned()),
                slicing: None,
                r#type: None,
                binding: None,
                extension: None,
            };

            let value_elem = ElementDefinition {
                id: format!("{}.value[x]", base_elem_ptr.id),
                path: format!("{}.value[x]", base_elem_ptr.path),
                slice_name: None,
                min: Some(0),
                max: Some("0".to_owned()),
                fixed_url: None,
                slicing: None,
                r#type: None,
                binding: None,
                extension: None,
            };

            let mut nested: Vec<ElementDefinition> = Vec::new();

            for (url, child) in complex_extension.extension {
                nested.append(&mut emit_nested(&extension_elem_ptr, url, child));
            }

            let mut res = Vec::new();
            // NOTE: FHIR prescribes specific order.
            res.push(base_elem);
            res.push(extension_elem);
            res.append(&mut nested);
            res.push(url_elem);
            res.push(value_elem);
            res
        }
    }
}

pub fn make_profiles(forest: &inverted::Forest) -> Vec<StructureDefinition> {
    let mut result: Vec<StructureDefinition> = Vec::new();
    for (rt, trie) in &forest.forest {
        let node = &trie.root;
        let profile = make_profile_for(rt, node);
        if let Some(profile) = profile {
            result.push(profile);
        }
    }

    result
}

pub fn make_profile_for(rt: &str, node: &inverted::NormalNode) -> Option<StructureDefinition> {
    make_profile_recursive(rt, &vec![], node)
}

pub fn make_profile_recursive(
    rt: &str,
    path: &[String],
    node: &inverted::NormalNode,
) -> Option<StructureDefinition> {
    let mut elements = make_profile_differential(rt, path, node);

    if elements.is_empty() {
        return None;
    }

    let mut differential = vec![ElementDefinition {
        id: rt.to_owned(),
        path: rt.to_owned(),
        slice_name: None,
        min: None,
        max: None,
        fixed_url: None,
        slicing: None,
        r#type: None,
        binding: None,
        extension: None,
    }];
    differential.append(&mut elements);

    Some(StructureDefinition {
        status: "active".to_string(),
        base_definition: format!("http://hl7.org/fhir/StructureDefinition/{rt}"),
        r#abstract: false,
        url: format!("http://legacy.aidbox.app/fhir/StructureDefinition/{rt}"),
        name: rt.to_owned(),
        derivation: "constraint".to_owned(),
        context: None,
        differential: StructureDefinitionDifferential {
            element: differential,
        },
        kind: "resource".to_owned(),
        r#type: rt.to_owned(),
    })
}

pub fn make_profile_differential(
    rt: &str,
    path: &[String],
    node: &inverted::NormalNode,
) -> Vec<ElementDefinition> {
    let mut result: Vec<ElementDefinition> = Vec::new();
    let extensions = match node {
        NormalNode::Complex(node) => Some(&node.extension),
        NormalNode::Inferred(node) => Some(&node.extension),
        _ => None,
    };
    if let Some(extensions) = extensions {
        let mut fhir_path = rt.to_owned();
        for path_component in path {
            fhir_path.push('.');
            fhir_path.push_str(&path_component);
        }
        fhir_path.push_str(".extension");

        for (url, ext) in extensions {
            let fce_property = ext.get_fce_property();

            let min = if ext.is_required() { Some(1) } else { None };
            let max = if ext.is_array() {
                Some("*".to_owned())
            } else {
                Some("1".to_owned())
            };

            result.push(ElementDefinition {
                id: format!("{fhir_path}:{fce_property}"),
                path: fhir_path.clone(),
                slice_name: Some(fce_property.to_owned()),
                min: min,
                max: max,
                fixed_url: None,
                slicing: None,
                r#type: Some(vec![ElementType {
                    code: "Extension".to_owned(),
                    target_profile: None,
                    profile: Some(vec![url.to_owned()]),
                }]),
                binding: None,
                extension: None,
            })
        }
    }

    let children = match node {
        NormalNode::Complex(node) => Some(&node.children),
        NormalNode::Inferred(node) => Some(&node.children),
        _ => None,
    };

    if let Some(children) = children {
        for (name, child) in children {
            let mut subpath = path.to_owned();
            subpath.push(name.to_owned());
            let mut subres = make_profile_differential(rt, &subpath, child);
            result.append(&mut subres);
        }
    }
    result
}
