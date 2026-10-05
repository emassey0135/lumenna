import AppKit
import Carbon.HIToolbox

/// A key and its modifiers, as Carbon registers it and as a person reads it.
struct Shortcut: Equatable {
    var keyCode: UInt32
    /// Carbon's modifier bits: `cmdKey`, `controlKey`, `optionKey`, `shiftKey`.
    var modifiers: UInt32
    /// The key as typed, such as "Y".
    var key: String

    /// "Control-Command-Y", the way macOS's own help writes a shortcut out — and the way
    /// VoiceOver reads it, which a symbol such as ⌃⌘ is not.
    var description: String {
        var parts: [String] = []
        if modifiers & UInt32(controlKey) != 0 { parts.append("Control") }
        if modifiers & UInt32(optionKey) != 0 { parts.append("Option") }
        if modifiers & UInt32(shiftKey) != 0 { parts.append("Shift") }
        if modifiers & UInt32(cmdKey) != 0 { parts.append("Command") }
        return (parts + [key]).joined(separator: "-")
    }

    /// From a key press, if it has a modifier that makes it usable from anywhere: Command or
    /// Control. A bare letter or a Shift-letter would stop that letter being typed at all.
    init?(event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags.contains(.command) || flags.contains(.control),
              let key = event.charactersIgnoringModifiers?.uppercased(), !key.isEmpty else { return nil }
        var modifiers: UInt32 = 0
        if flags.contains(.command) { modifiers |= UInt32(cmdKey) }
        if flags.contains(.control) { modifiers |= UInt32(controlKey) }
        if flags.contains(.option) { modifiers |= UInt32(optionKey) }
        if flags.contains(.shift) { modifiers |= UInt32(shiftKey) }
        self.init(keyCode: UInt32(event.keyCode), modifiers: modifiers, key: key == " " ? "Space" : key)
    }

    init(keyCode: UInt32, modifiers: UInt32, key: String) {
        self.keyCode = keyCode
        self.modifiers = modifiers
        self.key = key
    }

    static let controlCommand = UInt32(controlKey | cmdKey)

    /// What else answers to this, if anything — said before the person settles on it.
    ///
    /// Control-Option is VoiceOver's own modifier, so anything holding both is a VoiceOver
    /// command first. VOCR takes Control-Command with S and P, with L and I while moving
    /// through a scan, and Shift-Control-Command with more; macOS takes Control-Command with
    /// Q, F, D and Space.
    var conflict: String? {
        let control = modifiers & UInt32(controlKey) != 0
        let option = modifiers & UInt32(optionKey) != 0
        let shift = modifiers & UInt32(shiftKey) != 0
        let command = modifiers & UInt32(cmdKey) != 0
        if control && option {
            return "Control-Option is VoiceOver's modifier, so VoiceOver would take this first."
        }
        if control && command && !shift {
            switch key {
            case "Q": return "macOS uses this to lock the screen."
            case "F": return "macOS uses this for full screen."
            case "D": return "macOS uses this to look up a word."
            case "Space": return "macOS uses this for emoji and symbols."
            case "L", "I": return "VOCR uses this while you move through a scan, and not otherwise."
            case "S", "P": return "VOCR uses this."
            default: return nil
            }
        }
        if control && command && shift, ["S", "W", "V", "A", "R", "E", "U", "C", "Q", "3", "4"].contains(key) {
            return ["3", "4"].contains(key) ? "macOS uses this for screenshots." : "VOCR uses this."
        }
        return nil
    }
}

/// Shortcuts that work from any app: one brings the window back, one opens quick add.
///
/// Carbon's `RegisterEventHotKey`, because it needs no accessibility permission — an event
/// monitor for global keys does, and asking for it to make a shortcut work is asking too much.
/// The menu bar item is never the only way back to the window; this is the other way.
///
/// Control-Command, because Control-Option is VoiceOver's modifier. A global shortcut also
/// takes its key from whatever app is in front, so the defaults avoid the ones other apps are
/// known to use, and each can be changed or turned off in Settings. This Mac's alone: kept in
/// its own defaults, never synced.
enum HotKeys {
    enum Kind: String, CaseIterable {
        case summon, quickAdd

        var name: String {
            switch self {
            case .summon: "Show Lumenna"
            case .quickAdd: "Quick add a task"
            }
        }

        var standard: Shortcut {
            switch self {
            // L for Lumenna. VOCR has it too, but only while moving through a scan.
            case .summon: Shortcut(keyCode: UInt32(kVK_ANSI_L), modifiers: Shortcut.controlCommand, key: "L")
            case .quickAdd: Shortcut(keyCode: UInt32(kVK_ANSI_K), modifiers: Shortcut.controlCommand, key: "K")
            }
        }

        fileprivate var id: UInt32 {
            switch self {
            case .summon: 1
            case .quickAdd: 2
            }
        }
    }

    private static var actions: [UInt32: () -> Void] = [:]
    private static var references: [UInt32: EventHotKeyRef] = [:]
    private static var installed = false

    /// The shortcut for `kind`, or nil when it is turned off.
    static func shortcut(_ kind: Kind) -> Shortcut? {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: "hotkey.\(kind.rawValue).off") == nil else { return nil }
        guard let key = defaults.string(forKey: "hotkey.\(kind.rawValue).key") else { return kind.standard }
        return Shortcut(
            keyCode: UInt32(defaults.integer(forKey: "hotkey.\(kind.rawValue).code")),
            modifiers: UInt32(defaults.integer(forKey: "hotkey.\(kind.rawValue).modifiers")),
            key: key
        )
    }

    /// Changes a shortcut, or turns it off with nil, and registers it at once.
    static func set(_ kind: Kind, to shortcut: Shortcut?) {
        let defaults = UserDefaults.standard
        let name = kind.rawValue
        if let shortcut {
            defaults.removeObject(forKey: "hotkey.\(name).off")
            defaults.set(shortcut.key, forKey: "hotkey.\(name).key")
            defaults.set(Int(shortcut.keyCode), forKey: "hotkey.\(name).code")
            defaults.set(Int(shortcut.modifiers), forKey: "hotkey.\(name).modifiers")
        } else {
            defaults.set(true, forKey: "hotkey.\(name).off")
        }
        register(kind)
    }

    static func description(_ kind: Kind) -> String {
        shortcut(kind)?.description ?? "Off"
    }

    static func start(summon: @escaping () -> Void, quickAdd: @escaping () -> Void) {
        install()
        actions[Kind.summon.id] = summon
        actions[Kind.quickAdd.id] = quickAdd
        Kind.allCases.forEach(register)
    }

    private static func register(_ kind: Kind) {
        if let old = references.removeValue(forKey: kind.id) {
            UnregisterEventHotKey(old)
        }
        guard let shortcut = shortcut(kind) else { return }
        var reference: EventHotKeyRef?
        let hotKeyID = EventHotKeyID(signature: OSType(0x4C554D4E), id: kind.id) // "LUMN"
        if RegisterEventHotKey(shortcut.keyCode, shortcut.modifiers, hotKeyID, GetApplicationEventTarget(), 0, &reference) == noErr,
           let reference {
            references[kind.id] = reference
        }
    }

    private static func install() {
        guard !installed else { return }
        installed = true
        var type = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, _ in
            var hotKeyID = EventHotKeyID()
            GetEventParameter(
                event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                nil, MemoryLayout<EventHotKeyID>.size, nil, &hotKeyID
            )
            DispatchQueue.main.async { HotKeys.actions[hotKeyID.id]?() }
            return noErr
        }, 1, &type, nil, nil)
    }

    /// Asks for a new shortcut on `window`: the next key press with Command or Control is it,
    /// Escape leaves the old one. Says what else uses it, and asks before keeping a clash.
    static func record(_ kind: Kind, on window: NSWindow, done: @escaping () -> Void) {
        // Not answering to the old one while a new one is being pressed.
        if let old = references.removeValue(forKey: kind.id) { UnregisterEventHotKey(old) }
        let alert = NSAlert()
        alert.messageText = "New Shortcut for \(kind.name)"
        alert.informativeText = "Press the keys now, with Command or Control. With VoiceOver on, Control-Command keys reach Lumenna as they are. Escape keeps \(description(kind))."
        alert.addButton(withTitle: "Cancel")
        var monitor: Any?
        var finished = false
        let finish = { (shortcut: Shortcut?) in
            finished = true
            if let monitor { NSEvent.removeMonitor(monitor) }
            window.endSheet(alert.window)
            guard let shortcut else {
                register(kind)
                done()
                return
            }
            guard let conflict = shortcut.conflict else {
                set(kind, to: shortcut)
                Announcer.say("\(kind.name) is now \(shortcut.description)")
                done()
                return
            }
            DispatchQueue.main.async {
                let warning = NSAlert()
                warning.messageText = "\(shortcut.description) is taken"
                warning.informativeText = "\(conflict) Use it for \(kind.name) anyway?"
                warning.addButton(withTitle: "Keep \(description(kind))")
                warning.addButton(withTitle: "Use It Anyway")
                warning.beginSheetModal(for: window) { response in
                    if response == .alertSecondButtonReturn { set(kind, to: shortcut) } else { register(kind) }
                    done()
                }
            }
        }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == UInt16(kVK_Escape) {
                finish(nil)
                return nil
            }
            guard let shortcut = Shortcut(event: event) else { return nil }
            finish(shortcut)
            return nil
        }
        alert.beginSheetModal(for: window) { _ in
            // Cancel pressed, rather than a shortcut: the old one stands.
            guard !finished else { return }
            if let monitor { NSEvent.removeMonitor(monitor) }
            register(kind)
            done()
        }
    }
}
