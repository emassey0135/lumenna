//! The device's store, for what runs without the app's screens: reminders' alarms and their
//! receivers, a resident service giving reminders (BTSpeak's), a background sync, the watch's
//! link to its phone, backups. The [`App`] runs over
//! the same [`Device`], so the two share one connection and one sync service.
//!
//! Each method is the host's operation of the same name, typed through BoltFFI. Those after
//! the services are what the instrumented tests seed and check the store with; the screens go
//! through the app.
//!
//! [`App`]: crate::App

use std::sync::{Arc, Mutex, PoisonError};

use boltffi::export;
use lumenna_host::{Lumenna, PhoneLink, ReminderLock};
#[cfg(feature = "network")]
use lumenna_host::Reach;
use lumenna_records::proposals::{DayPins, DayPlan, Replan, ReplanChange, ReplanPins, TimeStyle};
use lumenna_records::reminders::DueReminders;
use lumenna_records::{
    BackupDone, Change, DeviceList, Filters, LumennaError, MoveTarget, NewBlock, Plan, Rows, SettingList, TaskEdit,
    TaskShown, Timer,
};
#[cfg(feature = "network")]
use lumenna_records::{LinkSynced, SyncReport, SyncStatus};

/// A stream of bytes to another device, which the platform carries: the watch's link to its phone.
#[cfg(feature = "network")]
#[export]
pub trait LinkStream: Send + Sync {
    /// Up to `max` bytes, waiting for some; empty once the stream has ended.
    fn read(&self, max: u32) -> Vec<u8>;
    /// Writes all of `bytes`; false if the stream has closed.
    fn write(&self, bytes: Vec<u8>) -> bool;
    /// Done writing.
    fn finish(&self);
}

/// A [`LinkStream`] of the platform's, as the host reads one.
#[cfg(feature = "network")]
struct Carried(Arc<dyn LinkStream>);

#[cfg(feature = "network")]
impl lumenna_host::LinkStream for Carried {
    fn read(&self, max: u32) -> Vec<u8> {
        self.0.read(max)
    }

    fn write(&self, bytes: Vec<u8>) -> bool {
        self.0.write(bytes)
    }

    fn finish(&self) {
        self.0.finish();
    }
}

/// This device's store, opened.
pub struct Device {
    pub(crate) lumenna: Arc<Lumenna>,
    /// The right to give this device's reminders, while this process holds it.
    reminders: Mutex<Option<Arc<ReminderLock>>>,
    /// The Apple Watch's exchange with its phone, on either side of it.
    link: Arc<PhoneLink>,
    /// This device kept in sync from here, by a process with no screens: BTSpeak's service.
    #[cfg(feature = "rpc")]
    syncing: Mutex<Option<Syncing>>,
}

/// The sync service a process with no screens runs, and the command surface it serves while
/// it holds the endpoint, as the daemon does.
#[cfg(feature = "rpc")]
struct Syncing {
    service: Arc<lumenna_host::SyncService>,
    commands: Arc<lumenna_host::rpc::CommandServer>,
}

impl Device {
    fn opened(lumenna: Arc<Lumenna>) -> Device {
        Device {
            link: PhoneLink::new(Arc::clone(&lumenna)),
            lumenna,
            reminders: Mutex::new(None),
            #[cfg(feature = "rpc")]
            syncing: Mutex::new(None),
        }
    }

    /// The device over a store already open: a client's own connection, served over RPC.
    #[cfg(feature = "rpc")]
    pub(crate) fn over(lumenna: Arc<Lumenna>) -> Device {
        Device::opened(lumenna)
    }
}

#[export]
impl Device {
    /// Opens the profile in `directory`, creating it on the first run.
    ///
    /// # Errors
    ///
    /// If the profile cannot be opened.
    pub fn new(directory: String) -> Result<Device, LumennaError> {
        Ok(Device::opened(Lumenna::open(&directory)?))
    }

    // The services.

    /// What fires before `until` (RFC 3339), for a system that schedules its own alarms.
    pub fn upcoming_reminders(&self, until: String) -> Result<DueReminders, LumennaError> {
        self.lumenna.upcoming_reminders(until)
    }

    /// What came due after `since` until `now` (RFC 3339; now when none).
    pub fn reminders_due(&self, since: String, now: Option<String>) -> Result<DueReminders, LumennaError> {
        self.lumenna.reminders_due(since, now)
    }

    /// What came due since this device last gave reminders, for a process that gives them as
    /// they come (BTSpeak's service): it takes the right to give them, one process at a time,
    /// and keeps it until [`Device::let_go_of_reminders`]. While another process has it,
    /// nothing is given and `elsewhere` says so, so ask again soon.
    pub fn deliver_reminders(&self, now: Option<String>) -> Result<DueReminders, LumennaError> {
        let mut held = self.reminders.lock().unwrap_or_else(PoisonError::into_inner);
        if held.is_none() {
            *held = self.lumenna.take_reminders()?;
        }
        match held.as_ref() {
            Some(lock) => self.lumenna.deliver_reminders(Arc::clone(lock), now),
            None => Ok(DueReminders::elsewhere()),
        }
    }

    /// Lets go of the right to give reminders, for another process to take.
    pub fn let_go_of_reminders(&self) {
        *self.reminders.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// What was missed while nothing ran, `shown` being what the platform showed (`key@occurrence`).
    pub fn reminders_missed(&self, shown: Vec<String>, kept: bool) -> Result<DueReminders, LumennaError> {
        self.lumenna.reminders_missed(shown, kept)
    }

    /// Dismisses a firing, on every device.
    pub fn dismiss_reminder(&self, key: String, occurrence: String) -> Result<Change, LumennaError> {
        self.lumenna.dismiss_reminder(key, occurrence)
    }

    /// Snoozes a firing, for `minutes` or the core's ten.
    pub fn snooze_reminder(&self, key: String, occurrence: String, minutes: Option<u32>) -> Result<Change, LumennaError> {
        self.lumenna.snooze_reminder(key, occurrence, minutes)
    }

    /// Carries every sitting a block's occurrence left short to its next block.
    pub fn carry_rest_forward(&self, block: String) -> Result<Change, LumennaError> {
        self.lumenna.carry_rest_forward(&block)
    }

    /// Takes the automatic backup if one is due, saying where it went.
    pub fn back_up_if_due(&self) -> Result<Option<String>, LumennaError> {
        self.lumenna.back_up_if_due()
    }

    /// Takes a backup now, into this device's backups.
    pub fn backup(&self) -> Result<BackupDone, LumennaError> {
        self.lumenna.backup(None)
    }

    /// A number that moves when another connection has written.
    pub fn outside_version(&self) -> Result<i64, LumennaError> {
        self.lumenna.outside_version()
    }

    /// Starts an exchange with the phone, from an Apple Watch: the first message to send.
    /// Whatever exchange was under way is dropped, here and, on reading this, on the phone.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn link_start(&self) -> Result<Vec<u8>, LumennaError> {
        self.link.start()
    }

    /// Takes the phone's reply to the last message sent, on an Apple Watch, and gives the next
    /// message to send, or nothing when the two stores agree.
    ///
    /// # Errors
    ///
    /// If the reply does not read, or the store refuses what it brought.
    pub fn link_take(&self, reply: Vec<u8>) -> Result<Option<Vec<u8>>, LumennaError> {
        self.link.take(reply)
    }

    /// Answers a message from the Apple Watch, on its phone: takes in what it brought, and
    /// gives the reply to send back.
    ///
    /// # Errors
    ///
    /// If the message does not read, or the store refuses what it brought.
    pub fn link_answer(&self, message: Vec<u8>) -> Result<Vec<u8>, LumennaError> {
        self.link.answer(message)
    }

    /// What the other side of the watch's link may lack, on either side: every change since the
    /// last exchange settled, for the system to deliver when the two are next in reach
    /// (`transferUserInfo`), with no reply. Nothing when there is nothing new.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn link_parcel(&self) -> Result<Option<Vec<u8>>, LumennaError> {
        self.link.parcel()
    }

    /// Takes in a parcel the other side of the watch's link handed on. Returns whether anything
    /// here changed.
    ///
    /// # Errors
    ///
    /// If the parcel does not read, or the store refuses what it brought.
    pub fn link_unparcel(&self, parcel: Vec<u8>) -> Result<bool, LumennaError> {
        self.link.unparcel(parcel)
    }

    /// Whether anything came in over the watch's link since this was last asked: it arrived on
    /// this device's own connection, which `outside_version` does not count, and the app ticks
    /// to take it in.
    pub fn link_took_in(&self) -> bool {
        self.link.took_in()
    }

    /// The store's heads as one word, which moves for every write, this device's app's own
    /// included: the watch sends what it wrote to its phone when it moves.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn heads(&self) -> Result<String, LumennaError> {
        self.lumenna.heads()
    }

    // What the tests seed and check the store with.

    /// Adds a task from a quick-add line.
    pub fn add_task(&self, text: String) -> Result<Change, LumennaError> {
        self.lumenna.add_task(&text)
    }

    /// Changes a task, sending only what changed.
    pub fn edit_task(&self, id: String, edit: TaskEdit) -> Result<Change, LumennaError> {
        self.lumenna.edit_task(&id, edit)
    }

    /// Moves a task under another, to a project, or to the top.
    pub fn move_task(&self, id: String, to: MoveTarget) -> Result<Change, LumennaError> {
        self.lumenna.move_task(&id, to)
    }

    /// Puts a task in the trash.
    pub fn trash_task(&self, id: String) -> Result<Change, LumennaError> {
        self.lumenna.trash_task(&id)
    }

    /// The tasks a query finds.
    pub fn list_tasks(&self, query: String) -> Result<Rows, LumennaError> {
        self.lumenna.list_tasks(&query)
    }

    /// One task, whole.
    pub fn show_task(&self, id: String) -> Result<TaskShown, LumennaError> {
        self.lumenna.show_task(&id)
    }

    /// Adds a project, under `parent` if given.
    pub fn add_project(&self, name: String, parent: Option<String>) -> Result<Change, LumennaError> {
        self.lumenna.add_project(&name, parent)
    }

    /// Archives a project.
    pub fn archive_project(&self, name: String) -> Result<Change, LumennaError> {
        self.lumenna.archive_project(&name)
    }

    /// Every project, as a tree.
    pub fn list_projects(&self) -> Result<Rows, LumennaError> {
        self.lumenna.list_projects()
    }

    /// Adds a label.
    pub fn add_label(&self, name: String) -> Result<Change, LumennaError> {
        self.lumenna.add_label(&name)
    }

    /// Every label.
    pub fn list_labels(&self) -> Result<Rows, LumennaError> {
        self.lumenna.list_labels()
    }

    /// The saved filters.
    pub fn list_filters(&self) -> Result<Filters, LumennaError> {
        self.lumenna.list_filters()
    }

    /// Adds a block.
    pub fn add_block(&self, block: NewBlock) -> Result<Change, LumennaError> {
        self.lumenna.add_block(block)
    }

    /// Every block series.
    pub fn list_blocks(&self) -> Result<Rows, LumennaError> {
        self.lumenna.list_blocks()
    }

    /// A day's plan: today, or a day as it is said.
    pub fn plan(&self, date: Option<String>) -> Result<Plan, LumennaError> {
        self.lumenna.plan(date)
    }

    /// When a new block on `date` starts unless the person says otherwise.
    pub fn new_block_start(&self, date: Option<String>) -> Result<String, LumennaError> {
        self.lumenna.new_block_start(date)
    }

    /// Puts a task in a block's occurrence, for `minutes` if given.
    pub fn assign(&self, task: String, block: String, date: Option<String>, minutes: Option<u32>) -> Result<Change, LumennaError> {
        self.lumenna.assign(&task, &block, date, minutes)
    }

    /// Stops a sitting's timer, logging `minutes` if given.
    pub fn stop_timer(&self, assignment: String, minutes: Option<u32>) -> Result<Timer, LumennaError> {
        self.lumenna.stop_timer(&assignment, minutes)
    }

    /// Adds a reminder to a task or block, as a phrase the core reads.
    pub fn add_reminder_saying(&self, target: String, phrase: String) -> Result<Change, LumennaError> {
        self.lumenna.add_reminder_saying(target, phrase)
    }

    /// Every setting, or one by its key.
    pub fn settings(&self, key: Option<String>) -> Result<SettingList, LumennaError> {
        self.lumenna.settings(key)
    }

    /// Changes a setting.
    pub fn set_setting(&self, key: String, value: String) -> Result<Change, LumennaError> {
        self.lumenna.set_setting(&key, &value)
    }

    /// The paired devices.
    pub fn devices(&self) -> Result<DeviceList, LumennaError> {
        self.lumenna.devices()
    }

    /// The rest of today worked out again, against `now` if given.
    pub fn replan(&self, style: TimeStyle, pins: ReplanPins, now: Option<String>) -> Result<Replan, LumennaError> {
        self.lumenna.replan(style, pins, now)
    }

    /// Makes the changes `accepted` of a replan, against `now` if given: the change made.
    ///
    /// # Errors
    ///
    /// If the day no longer matches the proposal, or the store refuses the change.
    pub fn apply_replan(&self, proposal: Replan, accepted: Vec<ReplanChange>, now: Option<String>) -> Result<Change, LumennaError> {
        self.lumenna.apply_replan(proposal, accepted, now).map(|replanned| replanned.change)
    }

    /// A whole day's plan, proposed.
    pub fn propose_day_plan(&self, date: Option<String>, style: TimeStyle, pins: DayPins, now: Option<String>) -> Result<DayPlan, LumennaError> {
        self.lumenna.propose_day_plan(date, style, pins, now)
    }
}

/// What reaches other devices, where this build has a network: not the Apple Watch's.
#[cfg(feature = "network")]
#[export]
impl Device {
    /// A round with every paired device now: on the sync service the app started, if it runs.
    pub fn sync_now(&self) -> Result<SyncReport, LumennaError> {
        self.lumenna.sync_now(Reach::Internet)
    }

    /// How syncing is going, and the devices.
    pub fn sync_status(&self) -> Result<SyncStatus, LumennaError> {
        self.lumenna.sync_status()
    }

    /// Syncs with the device at the other end of `stream`, introducing the two the first time.
    pub fn sync_over_link(&self, stream: Arc<dyn LinkStream>, device_name: String, platform: String) -> Result<LinkSynced, LumennaError> {
        self.lumenna.sync_over_link(Arc::new(Carried(stream)), &device_name, &platform)
    }
}

/// Keeping this device in sync from a process with no screens, as the daemon does: BTSpeak's
/// background service, which gives reminders too.
#[cfg(feature = "rpc")]
#[export]
impl Device {
    /// Keeps this device in sync until [`Device::stop_syncing`]: holds its endpoint, or waits its
    /// turn while another process holds it and takes over when that one stops, and while it
    /// holds it serves the command surface at the profile's address, so a round asked anywhere
    /// else on the device runs here. `process` is what a client is told is answering,
    /// `device_name` what a pairing asked for over it calls this device, and `platform` what
    /// this device says it runs. Whether this process holds the endpoint now; asking again
    /// while it keeps in sync changes nothing.
    ///
    /// # Errors
    ///
    /// If the endpoint cannot be opened.
    pub fn keep_in_sync(&self, process: String, device_name: String, platform: String) -> Result<bool, LumennaError> {
        let mut syncing = self.syncing.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(syncing) = syncing.as_ref() {
            return Ok(syncing.service.is_running());
        }
        let service = self.lumenna.start_sync(Reach::Internet, Arc::new(Quietly))?;
        let apps = Arc::new(crate::rpc::Apps { platform: platform.clone(), keeps_in_sync: None });
        let commands = self.lumenna.serve_commands(Arc::clone(&service), process, device_name, platform, Some(apps));
        let holding = service.is_running();
        *syncing = Some(Syncing { service, commands });
        Ok(holding)
    }

    /// Stops keeping this device in sync from here: gives up the profile's address, then the
    /// endpoint, for another process to take.
    pub fn stop_syncing(&self) {
        let syncing = self.syncing.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(syncing) = syncing {
            syncing.commands.stop();
            syncing.service.stop();
        }
    }
}

/// What arrives is not said: a process with no screens has nothing to redraw.
#[cfg(feature = "rpc")]
struct Quietly;

#[cfg(feature = "rpc")]
impl lumenna_host::SyncListener for Quietly {
    fn changed(&self) {}
}
