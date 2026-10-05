//! Reading and writing typed fields in an Automerge map.
//!
//! This is the whole of the CRDT mapping's mechanics, kept in one place so that
//! [`crate::records`] can read as a list of fields rather than as a pile of
//! `ScalarValue` matching.
//!
//! # Two rules shape everything here
//!
//! **Reads never fail on a wrong type.** Every getter returns `Option` and treats a missing
//! key, a null, and a value of the wrong shape identically. A document written by a newer
//! version, or by a client with a bug, has to load — tolerating nonsense is a
//! requirement rather than a kindness. Records that lose a *structurally* necessary field
//! are skipped and reported by [`crate::doc`]; nothing else is.
//!
//! **Writes only touch what changed.** Every setter takes the previous value alongside the
//! new one and does nothing when they match. This is not an optimization. Writing a field
//! that did not change still creates an operation, and that operation wins a
//! last-write-wins race against a concurrent edit from another device — so a client that
//! saved a whole record would silently revert every field of it that someone else had
//! touched. Passing `None` as the previous value writes everything, which is what creating
//! a record wants and only that.
//!
//! # Inline records
//!
//! A record keyed by something two devices can produce independently — an exception is
//! keyed by series and date, an acknowledgement by reminder and date — must not be a map of
//! its own. Both devices would create the map, merge would keep one, and every field written
//! into the other would be lost with it (`CLAUDE.md`'s rule about concurrent containers, one
//! level further down than the record sets of [`Writer::set_members`]).
//!
//! So those records are **inline**: each field is its own key in the collection,
//! `<record>/<field>`, and a nested map is spelt `<record>/<map>.<field>`. Nothing is ever
//! created, so there is nothing for merge to choose between, and two devices editing
//! different fields of the same occurrence both keep their edit. [`Reader`] and [`Writer`]
//! take a key prefix for this and otherwise behave identically, so a record's schema reads
//! the same either way.

use std::collections::BTreeSet;
use std::str::FromStr;

use automerge::transaction::Transactable;
use automerge::{ObjId, ObjType, ReadDoc, ScalarValue, Value};
use jiff::{Timestamp, civil};

use crate::error::Result;

/// A view of one map in a document.
pub(crate) struct Reader<'a, D: ReadDoc> {
    doc: &'a D,
    obj: ObjId,
    /// `None` for an ordinary map. For an inline record, what every key it owns starts with.
    prefix: Option<String>,
}

impl<'a, D: ReadDoc> Reader<'a, D> {
    /// Reads the map at `obj`.
    pub(crate) fn new(doc: &'a D, obj: ObjId) -> Self {
        Self { doc, obj, prefix: None }
    }

    /// Reads the keys of `obj` that start with `prefix`, as if they were a map of their own.
    pub(crate) fn inline(doc: &'a D, obj: ObjId, prefix: String) -> Self {
        Self { doc, obj, prefix: Some(prefix) }
    }

    fn full(&self, key: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}{key}"),
            None => key.to_owned(),
        }
    }

    /// The keys present, in document order. For an inline record, the first segment of
    /// each key under the prefix, once each.
    pub(crate) fn keys(&self) -> Vec<String> {
        match &self.prefix {
            None => self.doc.keys(&self.obj).collect(),
            Some(prefix) => {
                let mut keys: Vec<String> = self
                    .doc
                    .keys(&self.obj)
                    .filter_map(|k| {
                        let rest = k.strip_prefix(prefix.as_str())?;
                        Some(rest.split('.').next().unwrap_or(rest).to_owned())
                    })
                    .collect();
                keys.dedup();
                keys
            }
        }
    }

    fn scalar(&self, key: &str) -> Option<ScalarValue> {
        match self.doc.get(&self.obj, self.full(key)) {
            Ok(Some((Value::Scalar(s), _))) => Some(s.into_owned()),
            _ => None,
        }
    }

    /// A nested map or text object, if the key holds one of the expected type.
    fn object(&self, key: &str, expected: ObjType) -> Option<ObjId> {
        match self.doc.get(&self.obj, self.full(key)) {
            Ok(Some((Value::Object(actual), id))) if actual == expected => Some(id),
            _ => None,
        }
    }

    /// A nested map, as a reader of its own.
    pub(crate) fn map(&self, key: &str) -> Option<Reader<'a, D>> {
        match &self.prefix {
            None => self.object(key, ObjType::Map).map(|obj| Reader::new(self.doc, obj)),
            Some(_) => {
                let nested = format!("{}.", self.full(key));
                self.doc
                    .keys(&self.obj)
                    .any(|k| k.starts_with(&nested))
                    .then(|| Reader::inline(self.doc, self.obj.clone(), nested))
            }
        }
    }

    pub(crate) fn string(&self, key: &str) -> Option<String> {
        match self.scalar(key)? {
            ScalarValue::Str(s) => Some(s.to_string()),
            _ => None,
        }
    }

    pub(crate) fn bool(&self, key: &str) -> Option<bool> {
        match self.scalar(key)? {
            ScalarValue::Boolean(b) => Some(b),
            _ => None,
        }
    }

    pub(crate) fn int(&self, key: &str) -> Option<i64> {
        match self.scalar(key)? {
            ScalarValue::Int(i) => Some(i),
            ScalarValue::Uint(u) => i64::try_from(u).ok(),
            _ => None,
        }
    }

    /// An unsigned count. Negative and oversized values read as absent rather than
    /// saturating: a duration of -5 minutes is not better served by becoming 0.
    pub(crate) fn u32(&self, key: &str) -> Option<u32> {
        u32::try_from(self.int(key)?).ok()
    }

    pub(crate) fn f32(&self, key: &str) -> Option<f32> {
        match self.scalar(key)? {
            #[expect(clippy::cast_possible_truncation, reason = "stored as f64, used as f32")]
            ScalarValue::F64(f) => Some(f as f32),
            ScalarValue::Int(i) => Some(i as f32),
            _ => None,
        }
    }

    pub(crate) fn timestamp(&self, key: &str) -> Option<Timestamp> {
        match self.scalar(key)? {
            ScalarValue::Timestamp(ms) | ScalarValue::Int(ms) => {
                Timestamp::from_millisecond(ms).ok()
            }
            _ => None,
        }
    }

    /// Anything with a `FromStr`: identifiers, dates, times, order keys, zoned datetimes.
    ///
    /// Stored as text rather than packed integers, deliberately. A document is the thing
    /// that has to survive a decade of format changes across eleven clients, and a
    /// self-describing `2026-09-01` is debuggable in a way that a day count from an epoch
    /// nobody wrote down is not.
    pub(crate) fn parsed<T: FromStr>(&self, key: &str) -> Option<T> {
        self.string(key)?.parse().ok()
    }

    /// Text, character-merged. Absent or wrong-typed reads as empty.
    pub(crate) fn text(&self, key: &str) -> String {
        self.object(key, ObjType::Text)
            .and_then(|obj| self.doc.text(&obj).ok())
            .unwrap_or_default()
    }

    /// A set, stored as a map of member to `true`.
    ///
    /// Automerge has no set type, and this representation is the reason: adding and
    /// removing members are independent operations on independent keys, so two devices
    /// adding different labels to the same task both win. A list would order them for no
    /// reason and make concurrent inserts a merge problem; a delimited string would make
    /// every change a whole-field write.
    ///
    /// Members that no longer parse are dropped, which is the same treatment a member
    /// pointing at a deleted record gets.
    pub(crate) fn id_set<T: FromStr + Ord>(&self, key: &str) -> BTreeSet<T> {
        let Some(set) = self.map(key) else {
            return BTreeSet::new();
        };
        set.keys()
            .into_iter()
            .filter(|k| set.bool(k) == Some(true))
            .filter_map(|k| k.parse().ok())
            .collect()
    }
}

/// A cursor for writing one map in a transaction.
pub(crate) struct Writer<'a, T: Transactable> {
    tx: &'a mut T,
    obj: ObjId,
    /// As for [`Reader`]: `None` for an ordinary map, the key prefix for an inline record.
    prefix: Option<String>,
}

impl<'a, T: Transactable> Writer<'a, T> {
    /// Writes into the map at `obj`.
    pub(crate) fn new(tx: &'a mut T, obj: ObjId) -> Self {
        Self { tx, obj, prefix: None }
    }

    /// Writes each field as a key of `obj` starting with `prefix`, creating nothing.
    pub(crate) fn inline(tx: &'a mut T, obj: ObjId, prefix: String) -> Self {
        Self { tx, obj, prefix: Some(prefix) }
    }

    fn full(&self, key: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}{key}"),
            None => key.to_owned(),
        }
    }

    /// Gets or creates a nested map, returning a writer for it. Inline, a nested map is a
    /// longer prefix and never an object.
    pub(crate) fn map(&mut self, key: &str) -> Result<Writer<'_, T>> {
        if self.prefix.is_some() {
            let nested = format!("{}.", self.full(key));
            return Ok(Writer { tx: self.tx, obj: self.obj.clone(), prefix: Some(nested) });
        }
        let obj = match self.tx.get(&self.obj, key)? {
            Some((Value::Object(ObjType::Map), id)) => id,
            _ => self.tx.put_object(&self.obj, key, ObjType::Map)?,
        };
        Ok(Writer { tx: self.tx, obj, prefix: None })
    }

    /// Discards whatever is at `key` and returns a writer for a fresh map there.
    fn replace_map(&mut self, key: &str) -> Result<Writer<'_, T>> {
        if self.prefix.is_some() {
            self.clear(key)?;
            return self.map(key);
        }
        let obj = self.tx.put_object(&self.obj, key, ObjType::Map)?;
        Ok(Writer { tx: self.tx, obj, prefix: None })
    }

    /// Removes a key entirely — inline, along with every key nested under it.
    pub(crate) fn clear(&mut self, key: &str) -> Result<()> {
        let full = self.full(key);
        if self.tx.get(&self.obj, full.as_str())?.is_some() {
            self.tx.delete(&self.obj, full.as_str())?;
        }
        if self.prefix.is_some() {
            let nested = format!("{full}.");
            let under: Vec<String> =
                self.tx.keys(&self.obj).filter(|k| k.starts_with(&nested)).collect();
            for k in under {
                self.tx.delete(&self.obj, k.as_str())?;
            }
        }
        Ok(())
    }

    fn put(&mut self, key: &str, value: impl Into<ScalarValue>) -> Result<()> {
        let full = self.full(key);
        self.tx.put(&self.obj, full.as_str(), value)?;
        Ok(())
    }

    /// Writes a string if it differs from `before`.
    pub(crate) fn set_string(&mut self, key: &str, before: Option<&str>, now: &str) -> Result<()> {
        if before != Some(now) {
            self.put(key, now)?;
        }
        Ok(())
    }

    /// Writes a boolean if it differs from `before`.
    pub(crate) fn set_bool(&mut self, key: &str, before: Option<bool>, now: bool) -> Result<()> {
        if before != Some(now) {
            self.put(key, now)?;
        }
        Ok(())
    }

    /// Writes an integer if it differs from `before`.
    pub(crate) fn set_int(&mut self, key: &str, before: Option<i64>, now: i64) -> Result<()> {
        if before != Some(now) {
            self.put(key, now)?;
        }
        Ok(())
    }

    /// Writes an optional count, removing the key when it goes away.
    pub(crate) fn set_opt_u32(
        &mut self,
        key: &str,
        before: Option<Option<u32>>,
        now: Option<u32>,
    ) -> Result<()> {
        if before == Some(now) {
            return Ok(());
        }
        match now {
            Some(value) => self.put(key, i64::from(value))?,
            None => self.clear(key)?,
        }
        Ok(())
    }

    /// Writes a float if it differs from `before`.
    pub(crate) fn set_f32(&mut self, key: &str, before: Option<f32>, now: f32) -> Result<()> {
        if before != Some(now) {
            self.put(key, f64::from(now))?;
        }
        Ok(())
    }

    /// Writes anything with a `Display`, as text.
    pub(crate) fn set_str<V: PartialEq + ToString>(
        &mut self,
        key: &str,
        before: Option<&V>,
        now: &V,
    ) -> Result<()> {
        if before != Some(now) {
            self.put(key, now.to_string())?;
        }
        Ok(())
    }

    /// Writes an optional `Display` value, removing the key when it goes away.
    pub(crate) fn set_opt_str<V: PartialEq + ToString>(
        &mut self,
        key: &str,
        before: Option<Option<&V>>,
        now: Option<&V>,
    ) -> Result<()> {
        if before == Some(now) {
            return Ok(());
        }
        match now {
            Some(value) => self.put(key, value.to_string())?,
            None => self.clear(key)?,
        }
        Ok(())
    }

    /// Writes a timestamp, as Automerge's own timestamp scalar.
    pub(crate) fn set_time(
        &mut self,
        key: &str,
        before: Option<&Timestamp>,
        now: &Timestamp,
    ) -> Result<()> {
        if before != Some(now) {
            self.put(key, ScalarValue::Timestamp(now.as_millisecond()))?;
        }
        Ok(())
    }

    /// Writes an optional timestamp, removing the key when it goes away.
    pub(crate) fn set_opt_time(
        &mut self,
        key: &str,
        before: Option<Option<&Timestamp>>,
        now: Option<&Timestamp>,
    ) -> Result<()> {
        if before == Some(now) {
            return Ok(());
        }
        match now {
            Some(t) => self.put(key, ScalarValue::Timestamp(t.as_millisecond()))?,
            None => self.clear(key)?,
        }
        Ok(())
    }

    /// Updates a text object by diffing it against what is stored.
    ///
    /// The model carries notes as a plain `String` because core knows nothing about
    /// Automerge, and this is what makes that safe: `update_text` computes the minimal
    /// splice rather than replacing the object, so character-level merge survives
    /// even though the caller handed over the whole field.
    pub(crate) fn set_text(&mut self, key: &str, before: Option<&str>, now: &str) -> Result<()> {
        if before == Some(now) {
            return Ok(());
        }
        let key = self.full(key);
        let obj = match self.tx.get(&self.obj, key.as_str())? {
            Some((Value::Object(ObjType::Text), id)) => id,
            _ => self.tx.put_object(&self.obj, key.as_str(), ObjType::Text)?,
        };
        self.tx.update_text(&obj, now)?;
        Ok(())
    }

    /// Adds and removes set members individually.
    ///
    /// Never rewrites the set wholesale: two devices adding different labels to the same
    /// task must both keep theirs, and a whole-object replacement would make that a
    /// last-write-wins race that silently discards one of them.
    pub(crate) fn set_members<V: Ord + ToString>(
        &mut self,
        key: &str,
        before: Option<&BTreeSet<V>>,
        now: &BTreeSet<V>,
    ) -> Result<()> {
        let empty = BTreeSet::new();
        // On creation the map is made even when the set is empty. Otherwise the first two
        // devices to add a member each create the map, one creation wins, and the other's
        // member is lost with the object that held it — the same hazard `Doc::new` solves
        // for root collections, one level down.
        let creating = before.is_none();
        let before = before.unwrap_or(&empty);
        if before == now && !creating {
            return Ok(());
        }
        let mut set = self.map(key)?;
        for added in now.difference(before) {
            set.put(&added.to_string(), ScalarValue::Boolean(true))?;
        }
        for removed in before.difference(now) {
            set.clear(&removed.to_string())?;
        }
        Ok(())
    }

    /// Writes a nested record, replacing it wholesale when it differs.
    ///
    /// Wholesale replacement is right for the small composites the model has — a due date,
    /// an external reference, a delivery rule. Each is **one user decision**, so a
    /// concurrent change to another of its fields is not a merge to preserve but a
    /// disagreement to resolve, and last-write-wins over the whole object resolves it the
    /// way the user would expect. Sets get the opposite treatment for the opposite reason;
    /// see [`Writer::set_members`].
    pub(crate) fn set_nested<V, F>(
        &mut self,
        key: &str,
        before: Option<Option<&V>>,
        now: Option<&V>,
        write: F,
    ) -> Result<()>
    where
        V: PartialEq,
        F: FnOnce(&mut Writer<'_, T>, &V) -> Result<()>,
    {
        if before == Some(now) {
            return Ok(());
        }
        match now {
            Some(value) => {
                let mut nested = self.replace_map(key)?;
                write(&mut nested, value)?;
            }
            None => self.clear(key)?,
        }
        Ok(())
    }
}

/// Formats a civil date the way every date in a document is stored.
pub(crate) fn date_key(date: civil::Date) -> String {
    date.to_string()
}
