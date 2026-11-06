use super::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Grid = EditableTableState<Rows>;

pub struct DatasetStore {
    pub values: Vec<Vec<SharedString>>,
    pub(super) row_keys: Vec<SharedString>,
    pub(super) positions: HashMap<SharedString, usize>,
    pub(super) search_keys: Vec<Vec<String>>,
    pub(super) numeric_keys: Vec<Vec<Option<u64>>>,
    pub(super) revision: u64,
    pub(super) namespaces: BTreeMap<SharedString, usize>,
    pub(super) editors: Vec<gpui::WeakEntity<Grid>>,
    pub(super) cancel_controls: Vec<std::rc::Weak<Cell<Option<gpui::HitboxId>>>>,
    kind: ResourceKind,
    next_key: usize,
    saved_values: Vec<Vec<SharedString>>,
    saved_keys: Vec<SharedString>,
    saved_positions: HashMap<SharedString, usize>,
    inserted: BTreeSet<SharedString>,
    updated: BTreeSet<SharedString>,
    deleted: BTreeSet<SharedString>,
}

impl DatasetStore {
    pub fn new(data: &Dataset) -> Self {
        let values = data.rows.clone();
        let row_keys: Vec<SharedString> = (0..values.len()).map(|i| i.to_string().into()).collect();
        let positions = row_keys
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, key)| (key, i))
            .collect();
        let mut store = Self {
            saved_values: values.clone(),
            saved_keys: row_keys.clone(),
            values,
            row_keys,
            positions,
            search_keys: vec![],
            numeric_keys: vec![],
            revision: 0,
            namespaces: BTreeMap::new(),
            editors: vec![],
            cancel_controls: vec![],
            kind: data.kind,
            next_key: data.rows.len(),
            saved_positions: HashMap::new(),
            inserted: BTreeSet::new(),
            updated: BTreeSet::new(),
            deleted: BTreeSet::new(),
        };
        store.saved_positions = store.positions.clone();
        store.rebuild_caches();
        store
    }
    /// The workspace captures pointer presses before page controls. A Cancel
    /// press must reach the toolbar even when another split has an invalid edit.
    pub(crate) fn discarding_with_pointer(&self, window: &Window, cx: &App) -> bool {
        !window.last_input_was_keyboard()
            && (self
                .cancel_controls
                .iter()
                .filter_map(|control| control.upgrade())
                .any(|control| {
                    control
                        .get()
                        .is_some_and(|hitbox| hitbox.is_hovered(window))
                })
                || self
                    .open_editors()
                    .iter()
                    .any(|grid| grid.read(cx).context_menu_contains_pointer(window, cx)))
    }
    pub(crate) fn open_editors(&self) -> Vec<Entity<Grid>> {
        self.editors
            .iter()
            .filter_map(|editor| editor.upgrade())
            .collect()
    }
    /// Resolve a stable row identity even while retained view projections are
    /// catching up with a shared insert/delete/revert notification.
    pub(crate) fn row_values(&self, key: &SharedString) -> Option<&[SharedString]> {
        self.positions
            .get(key)
            .and_then(|index| self.values.get(*index))
            .map(Vec::as_slice)
    }
    pub(crate) fn draft_summary(&self, cx: &App) -> (usize, usize, usize, usize) {
        let (i, u, d) = self.changes();
        (
            i,
            u,
            d,
            self.open_editors()
                .iter()
                .filter(|grid| grid.read(cx).is_editing())
                .count(),
        )
    }
    pub(crate) fn discard_draft(&mut self) {
        self.revert();
    }
    fn rebuild_caches(&mut self) {
        self.positions = self
            .row_keys
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, key)| (key, i))
            .collect();
        self.search_keys = self
            .values
            .iter()
            .map(|row| row.iter().map(|v| v.to_lowercase()).collect())
            .collect();
        self.numeric_keys = self
            .values
            .iter()
            .map(|row| row.iter().map(|v| v.parse::<u64>().ok()).collect())
            .collect();
        self.namespaces.clear();
        if self.kind == ResourceKind::Kubernetes {
            for row in &self.values {
                *self.namespaces.entry(row[1].clone()).or_default() += 1;
            }
        }
    }
    pub(super) fn set_value(&mut self, row: usize, col: usize, value: SharedString) -> bool {
        if self.values[row][col] == value {
            return false;
        }
        if self.kind == ResourceKind::Kubernetes && col == 1 {
            let old = self.values[row][col].clone();
            if let Some(count) = self.namespaces.get_mut(&old) {
                *count -= 1;
                if *count == 0 {
                    self.namespaces.remove(&old);
                }
            }
            *self.namespaces.entry(value.clone()).or_default() += 1;
        }
        self.search_keys[row][col] = value.to_lowercase();
        self.numeric_keys[row][col] = value.parse::<u64>().ok();
        self.values[row][col] = value;
        if self.kind == ResourceKind::Database {
            let key = &self.row_keys[row];
            if let Some(saved) = self.saved_positions.get(key) {
                if self.values[row] == self.saved_values[*saved] {
                    self.updated.remove(key);
                } else {
                    self.updated.insert(key.clone());
                }
            }
        }
        true
    }
    pub(super) fn changes(&self) -> (usize, usize, usize) {
        (self.inserted.len(), self.updated.len(), self.deleted.len())
    }
    pub(super) fn dirty(&self) -> bool {
        self.changes() != (0, 0, 0)
    }
    pub(super) fn add(&mut self, data: &Dataset) -> SharedString {
        let id = self
            .values
            .iter()
            .filter_map(|row| row[0].parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .to_string();
        let mut row = vec![SharedString::default(); data.columns.len()];
        row[0] = id.into();
        for (col, key) in data.columns.iter().enumerate() {
            row[col] = match *key {
                "total" | "price" | "stock" => "0".into(),
                "currency" => "USD".into(),
                "status" if data.document.key.0.ends_with("/customers") => "Active".into(),
                "status" => "Pending".into(),
                "category" => "Hardware".into(),
                _ => row[col].clone(),
            };
        }
        let key: SharedString = self.next_key.to_string().into();
        self.next_key += 1;
        self.row_keys.push(key.clone());
        self.values.push(row);
        self.inserted.insert(key.clone());
        self.rebuild_caches();
        self.revision = self.revision.wrapping_add(1);
        key
    }
    pub(super) fn delete(&mut self, keys: &BTreeSet<SharedString>) {
        if !keys.iter().any(|key| self.positions.contains_key(key)) {
            return;
        }
        for key in keys {
            if !self.positions.contains_key(key) {
                continue;
            }
            if self.saved_positions.contains_key(key) {
                self.deleted.insert(key.clone());
            }
            self.inserted.remove(key);
            self.updated.remove(key);
        }
        let retained: Vec<_> = self
            .row_keys
            .drain(..)
            .zip(self.values.drain(..))
            .filter(|(key, _)| !keys.contains(key))
            .collect();
        (self.row_keys, self.values) = retained.into_iter().unzip();
        self.rebuild_caches();
        self.revision = self.revision.wrapping_add(1);
    }
    pub(super) fn save(&mut self) {
        self.saved_values = self.values.clone();
        self.saved_keys = self.row_keys.clone();
        self.saved_positions = self.positions.clone();
        self.inserted.clear();
        self.updated.clear();
        self.deleted.clear();
    }
    pub(super) fn revert(&mut self) {
        if !self.dirty() {
            return;
        }
        self.values = self.saved_values.clone();
        self.row_keys = self.saved_keys.clone();
        self.inserted.clear();
        self.updated.clear();
        self.deleted.clear();
        self.rebuild_caches();
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_track_crud_without_reusing_identity() {
        for data in Dataset::fixtures().iter().take(3) {
            let mut store = DatasetStore::new(data);
            let original = store.values.clone();
            let previous = store.values[0][1].clone();
            assert!(store.set_value(0, 1, "Updated".into()));
            assert_eq!(store.changes(), (0, 1, 0));
            store.set_value(0, 1, previous);
            assert!(!store.dirty());
            let key = store.add(data);
            assert_eq!(store.values.last().unwrap()[0].as_ref(), "1001");
            assert_eq!(store.changes(), (1, 0, 0));
            store.delete(&[key.clone()].into());
            assert!(!store.dirty());
            let next = store.add(data);
            assert_ne!(key, next);
            store.set_value(1000, 1, "New record".into());
            store.delete(&[SharedString::from("0")].into());
            assert_eq!(store.changes(), (1, 0, 1));
            assert_eq!(store.positions[&SharedString::from("1")], 0);
            store.revert();
            assert_eq!(store.values, original);
            assert_eq!(store.row_keys[0].as_ref(), "0");
            assert!(!store.dirty());
            assert_ne!(store.add(data), next);
            store.set_value(1000, 1, "Saved record".into());
            store.save();
            let saved = store.values.clone();
            store.set_value(0, 1, "Discarded".into());
            store.delete(&[SharedString::from("1")].into());
            store.revert();
            assert_eq!(store.values, saved);
            assert!(!store.dirty());
        }
    }

    #[gpui_kit::test]
    fn namespace_membership_name_search_and_stable_projection(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let data = Dataset::fixtures();
            let store = cx.new(|_| DatasetStore::new(&data[3]));
            assert_eq!(
                store
                    .read(cx)
                    .namespaces
                    .values()
                    .copied()
                    .collect::<Vec<_>>(),
                vec![500, 200, 300]
            );
            let mut rows = Rows::new(data[3].clone(), store.clone(), cx);
            rows.namespaces.insert("staging".into());
            rows.rebuild(cx);
            assert_eq!(rows.indices.len(), 300);
            rows.query = "staging".into();
            rows.rebuild(cx);
            assert!(
                rows.indices.is_empty(),
                "namespace text is not a name match"
            );
            rows.query.clear();
            rows.namespaces.insert("production".into());
            rows.rebuild(cx);
            assert_eq!(rows.indices.len(), 500);
            rows.namespaces.clear();
            rows.rebuild(cx);
            assert_eq!(rows.indices.len(), 1000);
            // Selected namespaces are independent of their availability in the source.
            rows.namespaces.insert("missing".into());
            rows.rebuild(cx);
            assert!(rows.indices.is_empty());
            assert!(rows.namespaces.contains("missing"));

            let store = cx.new(|_| DatasetStore::new(&data[0]));
            let mut rows = Rows::new(data[0].clone(), store.clone(), cx);
            rows.sort = Some((0, ColumnSort::Descending));
            rows.query = "yuki".into();
            rows.rebuild(cx);
            let key = rows.row_key(0, cx);
            let value = rows.value(&CellAddress::new(key.clone(), "name"), cx);
            store.update(cx, |store, _| {
                store.delete(&[SharedString::from("0")].into())
            });
            rows.rebuild(cx);
            assert_eq!(rows.row_key(0, cx), key);
            assert_eq!(rows.value(&CellAddress::new(key, "name"), cx), value);
            let reopened = Rows::new(data[0].clone(), store, cx);
            assert_eq!(reopened.indices.len(), 999);
        });
    }
}
