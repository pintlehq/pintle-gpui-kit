//! Session-owned navigation metadata and resource sources. Identity never follows labels.
use super::*;
use crate::data::{Dataset, DatasetStore, navigation};
use pintle_gpui_kit::shell::workspace::{Document, NavigationNode, ResourceKey};
use std::{collections::HashMap, rc::Rc};

pub struct Catalog {
    pub nodes: Vec<NavigationNode>,
    pub sources: HashMap<ResourceKey, (Rc<Dataset>, Entity<DatasetStore>)>,
    segments: HashMap<SharedString, SharedString>,
}

impl Catalog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let data = Dataset::fixtures();
        Self {
            nodes: navigation(&data),
            sources: data
                .into_iter()
                .map(|data| {
                    let store = cx.new(|_| DatasetStore::new(&data));
                    (data.document.key.clone(), (data, store))
                })
                .collect(),
            segments: [
                ("local".into(), "Local".into()),
                ("development".into(), "Development".into()),
            ]
            .into(),
        }
    }
    pub fn node(&self, id: &str) -> Option<&NavigationNode> {
        fn find<'a>(nodes: &'a [NavigationNode], id: &str) -> Option<&'a NavigationNode> {
            nodes.iter().find_map(|node| {
                if node.id == id {
                    Some(node)
                } else {
                    find(&node.children, id)
                }
            })
        }
        find(&self.nodes, id)
    }
    pub fn path(&self, id: &str) -> Vec<SharedString> {
        fn find(nodes: &[NavigationNode], id: &str) -> Option<Vec<SharedString>> {
            for node in nodes {
                if node.id == id {
                    return Some(vec![node.label.clone()]);
                }
                if let Some(mut path) = find(&node.children, id) {
                    path.insert(0, node.label.clone());
                    return Some(path);
                }
            }
            None
        }
        find(&self.nodes, id).unwrap_or_default()
    }
    pub fn editable(&self, id: &str) -> bool {
        fn contains(node: &NavigationNode, id: &str) -> bool {
            node.children
                .iter()
                .any(|child| child.id == id || contains(child, id))
        }
        self.nodes
            .iter()
            .filter(|root| root.id == "database" || root.id == "kubernetes")
            .any(|root| contains(root, id))
    }
    pub fn documents(&self, id: &str) -> Vec<Document> {
        fn collect(node: &NavigationNode, out: &mut Vec<Document>) {
            if let Some(doc) = &node.document {
                out.push(doc.clone());
            }
            for child in &node.children {
                collect(child, out);
            }
        }
        let mut out = vec![];
        if let Some(node) = self.node(id) {
            collect(node, &mut out);
        }
        out
    }
    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), SharedString> {
        if !self.editable(id) {
            return Err("This node cannot be renamed".into());
        }
        let name = name.trim();
        if name.is_empty()
            || name
                .chars()
                .any(|c| c == '/' || c == '\\' || c.is_control())
        {
            return Err("Enter a name without slashes or control characters".into());
        }
        fn siblings<'a>(nodes: &'a [NavigationNode], id: &str) -> Option<&'a [NavigationNode]> {
            if nodes.iter().any(|node| node.id == id) {
                Some(nodes)
            } else {
                nodes.iter().find_map(|node| siblings(&node.children, id))
            }
        }
        if siblings(&self.nodes, id).is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node.id != id && node.label.to_lowercase() == name.to_lowercase())
        }) {
            return Err("A sibling already has this name".into());
        }
        fn rename(nodes: &mut [NavigationNode], id: &str, name: &str) {
            for node in nodes {
                if node.id == id {
                    node.label = name.to_owned().into();
                }
                rename(&mut node.children, id, name);
            }
        }
        rename(&mut self.nodes, id, name);
        self.segments
            .insert(id.to_owned().into(), name.to_owned().into());
        fn refresh(
            nodes: &mut [NavigationNode],
            path: &[SharedString],
            segments: &HashMap<SharedString, SharedString>,
        ) {
            for node in nodes {
                if let Some(doc) = &mut node.document {
                    doc.title = node.label.clone();
                    doc.context = path
                        .iter()
                        .map(|s| s.as_ref())
                        .collect::<Vec<_>>()
                        .join(" / ")
                        .into();
                }
                let mut child_path = path.to_vec();
                child_path.push(segments.get(&node.id).unwrap_or(&node.label).clone());
                refresh(&mut node.children, &child_path, segments);
            }
        }
        for root in &mut self.nodes {
            if root.id != "foundation" {
                refresh(&mut root.children, &[], &self.segments);
            }
        }
        Ok(())
    }
    pub fn remove(&mut self, id: &str) -> bool {
        if !self.editable(id) {
            return false;
        }
        let documents = self.documents(id);
        fn remove(nodes: &mut Vec<NavigationNode>, id: &str) {
            nodes.retain(|node| node.id != id);
            for node in nodes {
                remove(&mut node.children, id);
            }
        }
        remove(&mut self.nodes, id);
        for document in documents {
            self.sources.remove(&document.key);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[gpui_kit::test]
    fn catalog_validation_identity_and_subtree_deletion(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let entity = cx.new(Catalog::new);
            entity.update(cx, |catalog, _| {
                let key = ResourceKey::from("db/local/shop/customers");
                let original = catalog.sources[&key].1.entity_id();
                for name in ["", "  ", "bad/name", "bad\\name", "bad\nname", " orders "] {
                    assert!(catalog.rename(&key.0, name).is_err(), "{name:?}");
                }
                assert!(!catalog.editable("database"));
                assert!(!catalog.editable("foundation"));
                assert!(!catalog.editable("nav-overview"));
                catalog.rename(&key.0, " Clients ").unwrap();
                assert_eq!(catalog.node(&key.0).unwrap().label.as_ref(), "Clients");
                assert_eq!(catalog.sources[&key].1.entity_id(), original);
                catalog.rename("local", "Remote").unwrap();
                assert_eq!(
                    catalog
                        .node(&key.0)
                        .unwrap()
                        .document
                        .as_ref()
                        .unwrap()
                        .context
                        .as_ref(),
                    "Remote / shop / public"
                );
                assert_eq!(
                    catalog
                        .path(&key.0)
                        .iter()
                        .map(|s| s.as_ref())
                        .collect::<Vec<_>>()
                        .join("/"),
                    "Database/Remote/shop/public/Clients"
                );
                assert!(!catalog.remove("kubernetes"));
                assert!(catalog.remove("public"));
                assert!(catalog.node(&key.0).is_none());
                assert!(!catalog.sources.contains_key(&key));
                assert_eq!(catalog.sources.len(), 3);
                assert!(catalog.node("shop").is_some());
                assert!(catalog.node("k8s/development/pods").is_some());
            });
        });
    }
}
