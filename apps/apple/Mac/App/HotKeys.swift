import AppKit
import Carbon.HIToolbox

/// Shortcuts that work from any app (§16.2): one brings the window back, one opens quick add.
///
/// Carbon's `RegisterEventHotKey`, because it needs no accessibility permission — an event
/// monitor for global keys does, and asking for it to make a shortcut work is asking too much.
/// The tray is never the only way back to the window; this is the other way.
enum HotKeys {
    /// Control-Option-L.
    static let summonDescription = "Control-Option-L"
    /// Control-Option-Space.
    static let quickAddDescription = "Control-Option-Space"

    private static var actions: [UInt32: () -> Void] = [:]
    private static var installed = false
    private static var references: [EventHotKeyRef] = []

    static func register(summon: @escaping () -> Void, quickAdd: @escaping () -> Void) {
        install()
        add(id: 1, key: UInt32(kVK_ANSI_L), modifiers: UInt32(controlKey | optionKey), action: summon)
        add(id: 2, key: UInt32(kVK_Space), modifiers: UInt32(controlKey | optionKey), action: quickAdd)
    }

    private static func add(id: UInt32, key: UInt32, modifiers: UInt32, action: @escaping () -> Void) {
        actions[id] = action
        var reference: EventHotKeyRef?
        let hotKeyID = EventHotKeyID(signature: OSType(0x4C554D4E), id: id) // "LUMN"
        if RegisterEventHotKey(key, modifiers, hotKeyID, GetApplicationEventTarget(), 0, &reference) == noErr,
           let reference {
            references.append(reference)
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
}
