import LumennaApp

// The surface's records as the app's, while screens move from the one onto the other: the same
// shapes, from the same Rust, through UniFFI and BoltFFI. Gone with the surface's bindings.

extension LumennaApp.Place {
    init(_ place: Place) {
        switch place {
        case .today: self = .today
        case .tasks: self = .tasks
        case let .project(name): self = .project(name)
        case let .label(name): self = .label(name)
        case let .filter(name, query): self = .filter(name: name, query: query)
        case .blocks: self = .blocks
        case .trash: self = .trash
        }
    }
}

/// The surface's action, for code that sees both modules (the hosted tests), where `Action` is
/// either.
typealias SurfaceAction = Action
