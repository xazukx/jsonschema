use std::collections::{HashMap, HashSet};

use crate::{compiler::CompileError, draft::*, util::*};

use serde_json::Value;
use url::Url;

pub struct Root {
    pub draft: &'static Draft,
    pub resources: HashMap<JsonPointer, Resource>, // ptr => _
    pub url: Url,
    pub meta_vocabs: Option<Vec<String>>,
}

impl Root {
    pub fn has_vocab(&self, name: &str) -> bool {
        if self.draft.version < 2019 || name == "core" {
            return true;
        }
        if let Some(vocabs) = &self.meta_vocabs {
            return vocabs.iter().any(|s| s == name);
        }
        self.draft.default_vocabs.contains(&name)
    }

    fn resolve_fragment_in(&self, frag: &Fragment, res: &Resource) -> Result<UrlPtr, CompileError> {
        let ptr = match frag {
            Fragment::Anchor(anchor) => {
                let Some(ptr) = res.anchors.get(anchor) else {
                    return Err(CompileError::AnchorNotFound {
                        url: self.url.to_string(),
                        reference: UrlFrag::format(&res.id, frag.as_str()),
                    });
                };
                ptr.clone()
            }
            Fragment::JsonPointer(ptr) => res.ptr.concat(ptr),
        };
        Ok(UrlPtr {
            url: self.url.clone(),
            ptr,
        })
    }

    pub fn resolve_fragment(&self, frag: &Fragment) -> Result<UrlPtr, CompileError> {
        let res = self.resources.get("").ok_or(CompileError::Bug(
            format!("no root resource found for {}", self.url).into(),
        ))?;
        self.resolve_fragment_in(frag, res)
    }

    // resolves `UrlFrag` to `UrlPtr` from root.
    // returns `None` if it is external.
    pub fn resolve(&self, uf: &UrlFrag) -> Result<Option<UrlPtr>, CompileError> {
        let res = {
            if uf.url == self.url {
                self.resources.get("").ok_or(CompileError::Bug(
                    format!("no root resource found for {}", self.url).into(),
                ))?
            } else {
                // look for resource with id==uf.url
                let Some(res) = self.resources.values().find(|res| res.id == uf.url) else {
                    return Ok(None); // external url
                };
                res
            }
        };

        self.resolve_fragment_in(&uf.frag, res).map(Some)
    }

    pub fn resource(&self, ptr: &JsonPointer) -> &Resource {
        let mut ptr = ptr.as_str();
        loop {
            if let Some(res) = self.resources.get(ptr) {
                return res;
            }
            let Some((prefix, _)) = ptr.rsplit_once('/') else {
                break;
            };
            ptr = prefix;
        }
        self.resources.get("").expect("root resource should exist")
    }

    pub fn base_url(&self, ptr: &JsonPointer) -> &Url {
        &self.resource(ptr).id
    }

    pub fn add_subschema(
        &mut self,
        doc: &Value,
        ptr: &JsonPointer,
    ) -> Result<(), CompileError> {
        let v = ptr.lookup(doc, &self.url)?;
        let base_url = self.base_url(ptr).clone();
        self.draft
            .collect_resources(v, &base_url, ptr.clone(), &self.url, &mut self.resources)?;

        // collect anchors
        if !self.resources.contains_key(ptr) {
            let res = self.resource(ptr);
            if let Some(res) = self.resources.get_mut(&res.ptr.clone()) {
                self.draft.collect_anchors(v, ptr, res, &self.url)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Resource {
    pub ptr: JsonPointer, // from root
    pub id: Url,
    pub anchors: HashMap<Anchor, JsonPointer>, // anchor => ptr
    pub dynamic_anchors: HashSet<Anchor>,
}

impl Resource {
    pub fn new(ptr: JsonPointer, id: Url) -> Self {
        Self {
            ptr,
            id,
            anchors: HashMap::new(),
            dynamic_anchors: HashSet::new(),
        }
    }
}
