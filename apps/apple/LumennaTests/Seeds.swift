import LumennaApp

/// A block to seed a test's store with, through the device: BoltFFI's records have no defaults,
/// so what a test leaves unsaid is left to the kind.
func newBlock(title: String, at: String, minutes: UInt32, date: String?, kind: String = "work", repeat repeating: String? = nil) -> LumennaApp.NewBlock {
    LumennaApp.NewBlock(
        title: title, at: at, minutes: minutes, date: date, kind: kind, repeat: repeating, notes: nil, acceptsTasks: nil,
        countsCapacity: nil, anchored: nil, minMinutes: nil, taskFilter: nil, until: nil, colour: nil
    )
}
