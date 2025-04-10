use std::collections::{HashMap, HashSet};

use crate::{compiler::CompileError, draft::*, loader::DefaultUrlLoader, root::Root, util::*};

use serde_json::Value;
use url::Url;

// --

pub struct Roots {
    pub default_draft: &'static Draft,
    map: HashMap<Url, Root>,
    pub loader: DefaultUrlLoader,
}

impl Roots {
    fn new() -> Self {
        Self {
            default_draft: latest(),
            map: Default::default(),
            loader: DefaultUrlLoader::new(),
        }
    }
}

impl Default for Roots {
    fn default() -> Self {
        Self::new()
    }
}

impl Roots {
    pub fn get(&self, url: &Url) -> Option<&Root> {
        self.map.get(url)
    }

    pub fn resolve_fragment(&mut self, uf: UrlFrag) -> Result<UrlPtr, CompileError> {
        self.or_load(uf.url.clone())?;
        let Some(root) = self.map.get(&uf.url) else {
            return Err(CompileError::Bug("or_load didn't add".into()));
        };
        root.resolve_fragment(&uf.frag)
    }

    pub fn ensure_subschema(&mut self, up: &UrlPtr) -> Result<(), CompileError> {
        self.or_load(up.url.clone())?;
        let Some(root) = self.map.get_mut(&up.url) else {
            return Err(CompileError::Bug("or_load didn't add".into()));
        };
        if !root.draft.is_subschema(up.ptr.as_str()) {
            let doc = self.loader.load(&root.url)?;
            let v = up.ptr.lookup(doc, &up.url)?;
            root.draft.validate(up, v)?;
            root.add_subschema(doc, &up.ptr)?;
        }
        Ok(())
    }

    pub fn or_load(&mut self, url: Url) -> Result<(), CompileError> {
        debug_assert!(url.fragment().is_none(), "trying to add root with fragment");
        if self.map.contains_key(&url) {
            return Ok(());
        }
        let doc = self.loader.load(&url)?;
        let r = self.create_root(url.clone(), doc)?;
        self.map.insert(url, r);
        Ok(())
    }

    pub fn create_root(&self, url: Url, doc: &Value) -> Result<Root, CompileError> {
        let draft = {
            let up = UrlPtr {
                url: url.clone(),
                ptr: "".into(),
            };
            self.loader
                .get_draft(&up, doc, self.default_draft, HashSet::new())?
        };
        let vocabs = self.loader.get_meta_vocabs(doc, draft)?;
        let resources = {
            let mut m = HashMap::default();
            draft.collect_resources(doc, &url, "".into(), &url, &mut m)?;
            m
        };

        if !matches!(url.host_str(), Some("json-schema.org")) {
            draft.validate(
                &UrlPtr {
                    url: url.clone(),
                    ptr: "".into(),
                },
                doc,
            )?;
        }

        Ok(Root {
            draft,
            resources,
            url: url.clone(),
            meta_vocabs: vocabs,
        })
    }

    pub fn insert(&mut self, roots: &mut HashMap<Url, Root>) {
        self.map.extend(roots.drain());
    }
}
