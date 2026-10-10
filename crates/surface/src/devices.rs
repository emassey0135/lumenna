//! The paired devices: the list, renaming one, unpairing one.
//!
//! Built with or without Iroh: the Apple Watch, which syncs through its iPhone and is in no
//! device list, still holds the synced list, and can name and unpair the devices in it as
//! any of them can. Without Iroh nothing is "this device", and a device's status says only
//! its version, since the watch has never synced with it directly.

use lumenna_core::edit;
use lumenna_core::id::NodeId;
use lumenna_core::model::Device;

use crate::error::{LumennaError, Result};
use crate::tasks::record_or;
use crate::types::{Announced, Change, DeviceList, DeviceView};
use crate::words::count_line;
use crate::{Lumenna, repaired};

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// The paired devices, this one first.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn devices(&self) -> Result<DeviceList> {
        let devices = self.device_views(self.this_device()?)?;
        Ok(DeviceList {
            announcement: count_line(devices.len(), "paired device"),
            notices: Vec::new(),
            devices,
        })
    }

    /// Renames a device, found by name or the start of its identifier.
    ///
    /// # Errors
    ///
    /// If no device, or more than one, matches.
    pub fn rename_device(&self, device: &str, name: &str) -> Result<Change> {
        let found = self.find_device(device)?;
        self.told(|store| {
            let change = edit::rename_device(&repaired(store), found.node_id, name)?;
            record_or(store, &change, "it already has that name")
        })
    }

    /// Stops syncing with a device. It keeps what it already has.
    ///
    /// # Errors
    ///
    /// If no device, or more than one, matches; or it is this device.
    pub fn unpair_device(&self, device: &str) -> Result<Change> {
        let found = self.find_device(device)?;
        if Some(found.node_id) == self.this_device()? {
            return Err(LumennaError::new(
                "this device cannot unpair itself; unpair it from one of your other devices",
            ));
        }
        let change = self.told(|store| {
            let change = edit::unpair_device(&repaired(store), found.node_id)?;
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })?;
        // Say plainly what unpairing does not do.
        Ok(change.note(format!(
            "{} keeps everything it already has. Unpairing is for a device you replaced; if it \
             was lost or stolen, unpairing alone does not take your data back from it",
            found.name
        )))
    }
}

impl Lumenna {
    /// This device's identity among the paired, or none where there is no Iroh endpoint.
    fn this_device(&self) -> Result<Option<NodeId>> {
        #[cfg(feature = "sync")]
        {
            Ok(Some(crate::sync::this_node(&self.shared())?))
        }
        #[cfg(not(feature = "sync"))]
        {
            Ok(None)
        }
    }

    /// Every paired device as a view, this one (`me`) first, then by name.
    pub(crate) fn device_views(&self, me: Option<NodeId>) -> Result<Vec<DeviceView>> {
        let (snapshot, peers) = self.with(|store| Ok((repaired(store), store.peers()?)))?;
        let when = |millis: i64| {
            jiff::Timestamp::from_millisecond(millis).map_or_else(|_| String::new(), |t| t.to_string())
        };
        let mut views: Vec<DeviceView> = snapshot
            .devices
            .values()
            .map(|device| {
                let status = peers.iter().find(|p| p.node_id == device.node_id.to_string());
                let mut view = DeviceView {
                    name: device.name.clone(),
                    platform: device.platform.clone(),
                    node_id: device.node_id.to_string(),
                    this_device: Some(device.node_id) == me,
                    paired_at: device.paired_at.to_string(),
                    last_attempt: status.and_then(|s| s.last_attempt).map(when),
                    last_success: status.and_then(|s| s.last_success).map(when),
                    last_error: status.and_then(|s| s.last_error.clone()),
                    schema_version: device.schema,
                    status: Vec::new(),
                };
                view.status = crate::words::device_status(&view, jiff::Timestamp::now(), me.is_some());
                view
            })
            .collect();
        views.sort_by(|a, b| b.this_device.cmp(&a.this_device).then(a.name.cmp(&b.name)));
        Ok(views)
    }

    fn find_device(&self, input: &str) -> Result<Device> {
        let snapshot = self.with(|store| Ok(repaired(store)))?;
        let lowered = input.to_lowercase();
        let matches: Vec<&Device> = snapshot
            .devices
            .values()
            .filter(|d| d.name.to_lowercase() == lowered || d.node_id.to_string().starts_with(&lowered))
            .collect();
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(LumennaError::new(format!("no paired device called '{input}'"))),
            _ => Err(LumennaError::new(format!(
                "'{input}' matches more than one device; give more of its identifier"
            ))),
        }
    }
}
