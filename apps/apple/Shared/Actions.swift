import Foundation

/// What can be done to a row, in the order offered, as the iPhone, iPad and watch all offer
/// it: which actions apply when, their spoken names, and the words their questions ask. Each
/// app turns these into swipe actions, menus or buttons and runs them; none decides which
/// apply by itself, so the apps cannot drift apart on it.

/// What can be done to a row of the day.
enum DayAction: Hashable {
    case assignTask, edit, cancelThisDay, restoreThisDay, deleteBlock
    case startTimer, resumeTimer, pauseTimer, stopTimer, plannedLength, logMinutes, unassign
    case addBlockHere

    /// Its spoken name: "Delete Block", not "Delete".
    var title: String {
        switch self {
        case .assignTask: "Assign Task"
        case .edit: "Edit"
        case .cancelThisDay: "Cancel This Day"
        case .restoreThisDay: "Restore This Day"
        case .deleteBlock: "Delete Block"
        case .startTimer: "Start Timer"
        case .resumeTimer: "Resume Timer"
        case .pauseTimer: "Pause Timer"
        case .stopTimer: "Stop Timer"
        case .plannedLength: "Planned Length"
        case .logMinutes: "Log Minutes"
        case .unassign: "Unassign"
        case .addBlockHere: "Add Block Here"
        }
    }

    /// Whether it is hard to take back, so shown as such.
    var destructive: Bool {
        self == .deleteBlock || self == .unassign
    }

    /// A block's: tasks go in one that takes them, a repeating one can skip a day, a day
    /// changed from the series can go back to it.
    static func of(_ block: PlanBlock) -> [DayAction] {
        var actions: [DayAction] = []
        if block.acceptsTasks { actions.append(.assignTask) }
        actions.append(.edit)
        if block.repeats { actions.append(.cancelThisDay) }
        if block.changedForThisDay { actions.append(.restoreThisDay) }
        actions.append(.deleteBlock)
        return actions
    }

    /// A sitting's: start, pause and stop — a paused sitting is still in progress, and
    /// stopping either a running or a paused one ends it — then its length and its time.
    static func of(_ sitting: PlanAssignment) -> [DayAction] {
        var actions: [DayAction] = [sitting.running ? .pauseTimer : (sitting.status == "paused" ? .resumeTimer : .startTimer)]
        if sitting.running || sitting.status == "paused" { actions.append(.stopTimer) }
        return actions + [.plannedLength, .logMinutes, .unassign]
    }

    /// Free time's.
    static let ofFreeTime: [DayAction] = [.addBlockHere]
    /// A day cancelled from a repeating block's.
    static let ofCancelled: [DayAction] = [.restoreThisDay]

    /// What deleting a block asks, which differs for one that repeats.
    static func deleting(_ block: PlanBlock) -> String {
        block.repeats
            ? "Every occurrence goes, not only this day. To skip one day, cancel it instead."
            : "It goes to the trash with its assignments."
    }

    /// What logging minutes asks.
    static let loggingMinutes = "The whole of this sitting, replacing what is logged."
}

/// What can be done to a project, a label or a saved filter, wherever one is listed.
enum PlaceAction: Hashable {
    case rename, moveUp, moveDown, moveUnder, addProjectInside, weight, archive, unarchive
    case mergeInto, colour, changeQuery, delete

    var title: String {
        switch self {
        case .rename: "Rename"
        case .moveUp: "Move Up"
        case .moveDown: "Move Down"
        case .moveUnder: "Move Under"
        case .addProjectInside: "Add Project Inside"
        case .weight: "Weight"
        case .archive: "Archive"
        case .unarchive: "Unarchive"
        case .mergeInto: "Merge Into"
        case .colour: "Colour"
        case .changeQuery: "Change Query"
        case .delete: "Delete"
        }
    }

    var destructive: Bool { self == .delete }

    /// A project's: rename, reorder, nest, weigh, archive, delete.
    static func ofProject(archived: Bool) -> [PlaceAction] {
        [.rename, .moveUp, .moveDown, .moveUnder, .addProjectInside, .weight, archived ? .unarchive : .archive, .delete]
    }

    /// A label's: rename, reorder, merge, colour, delete.
    static let ofLabel: [PlaceAction] = [.rename, .moveUp, .moveDown, .mergeInto, .colour, .delete]
    /// A saved filter's: rename, change its query, reorder, delete.
    static let ofFilter: [PlaceAction] = [.rename, .changeQuery, .moveUp, .moveDown, .delete]

    /// What deleting a project asks, offering both answers.
    static let deletingProject = "Its tasks can go to the trash with it, or move to the Inbox."
    static let deleteAndTrash = "Delete and Trash Its Tasks"
    static let deleteAndKeep = "Delete and Keep Its Tasks"
    /// What deleting a label asks.
    static let deletingLabel = "Tasks wearing it stay; they just stop showing it."
    /// What a label's colour takes.
    static let colourHelp = "A colour name, such as red or teal, or none. The name always shows too."
    /// What a project's weight takes.
    static let weightHelp = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again."
}

/// What can be done to a paired device.
enum DeviceAction: Hashable {
    case rename, unpair

    var title: String {
        switch self {
        case .rename: "Rename"
        case .unpair: "Unpair"
        }
    }

    var destructive: Bool { self == .unpair }

    /// A device cannot unpair itself, so that is not offered on its own row.
    static func of(thisDevice: Bool) -> [DeviceAction] {
        thisDevice ? [.rename] : [.rename, .unpair]
    }

    /// What unpairing asks: what it does, and what it does not.
    static let unpairing = "It stops syncing with your devices but keeps everything it already has. Unpairing is for a device you replaced; it does not take data back from a lost one."
}
