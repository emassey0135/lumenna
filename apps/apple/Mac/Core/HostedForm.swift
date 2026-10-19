import AppKit
import SwiftUI

/// A SwiftUI form hosted in AppKit, as VoiceOver meets it: one named scroll area.
///
/// Left alone, the hosting view is a group, and naming it — which the audit asks for — put
/// a "Planning" group around SwiftUI's own scroll area, two levels to interact through
/// before reaching a setting. So the hosting view steps out of the tree and its name goes to
/// the scroll area, which is what is actually there. It has to say so itself: told from
/// outside, a hosting view goes on reporting itself as a group.
class HostedForm<Content: View>: NSViewController {
    private let name: String
    let host: FlatHostingView<Content>

    init(_ name: String, rootView: Content) {
        self.name = name
        host = FlatHostingView(rootView: rootView)
        super.init(nibName: nil, bundle: nil)
        title = name
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        view = host
    }

    private var named = false

    /// The form's scroll view exists only once SwiftUI has laid it out, which can be after
    /// the view appears; so this tries on appearing and after each layout until it is there.
    /// Falling back to naming the hosting view would bring back the group around the scroll
    /// area, so it never does.
    override func viewDidAppear() {
        super.viewDidAppear()
        nameScrollArea()
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        nameScrollArea()
    }

    private func nameScrollArea() {
        guard !named, let scroll = firstScrollView(in: host) else { return }
        scroll.setAccessibilityLabel(name)
        named = true
    }
}

/// A hosting view that is not itself an accessibility element, so its contents sit directly
/// in whatever holds it.
final class FlatHostingView<Content: View>: NSHostingView<Content> {
    override func isAccessibilityElement() -> Bool { false }
}

private func firstScrollView(in view: NSView) -> NSScrollView? {
    for subview in view.subviews {
        if let scroll = subview as? NSScrollView { return scroll }
        if let found = firstScrollView(in: subview) { return found }
    }
    return nil
}

extension NSView {
    /// Makes this view first responder when focus is nowhere: as the window opens, or when the
    /// pane it was in was replaced. A list coming into the middle pane as the person moves
    /// through the sidebar leaves them there; taking focus, it stopped the arrows at the first
    /// place they came to.
    func takeFocusIfNowhere() {
        guard let window else { return }
        let current = window.firstResponder
        let nowhere = current == nil || current === window || (current as? NSView)?.window !== window
        if nowhere { window.makeFirstResponder(self) }
    }
}

/// Gives a view focus as the mouse goes down on it, before AppKit handles the click: a SwiftUI
/// list's own outline does not take focus for a click on the row already selected, so Space
/// after it went to the sidebar, and a recognizer that gave it on the click's end came after a
/// key pressed straight away.
final class FocusOnClick {
    private weak var view: NSView?
    private var monitor: Any?

    init(_ view: NSView) {
        self.view = view
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let view = self?.view, let window = view.window, event.window === window,
                  window.firstResponder !== view else { return event }
            let point = view.convert(event.locationInWindow, from: nil)
            if view.bounds.contains(point) { window.makeFirstResponder(view) }
            return event
        }
    }

    deinit {
        if let monitor { NSEvent.removeMonitor(monitor) }
    }
}

/// The keys a list answers on its row in hand.
enum ListKey { case space, delete, returnKey }

/// Space, Delete and Return on a list, taken as AppKit hands them to the window while the
/// list's outline, or a view inside it, is first responder. SwiftUI's own key handlers waited on its focus, which
/// follows the first responder a moment later: a key pressed straight after a click was lost.
final class ListKeys {
    private weak var view: NSView?
    private var monitor: Any?

    /// `handle` answers whether it took the key.
    init(_ view: NSView, handle: @escaping (ListKey) -> Bool) {
        self.view = view
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let view = self?.view, let window = view.window, event.window === window,
                  let responder = window.firstResponder as? NSView, responder.isDescendant(of: view),
                  event.modifierFlags.intersection([.command, .option, .control, .shift]).isEmpty
            else { return event }
            let key: ListKey? = switch event.keyCode {
            case 49: .space
            case 36, 76: .returnKey
            case 51, 117: .delete
            default: nil
            }
            guard let key, handle(key) else { return event }
            return nil
        }
    }

    deinit {
        if let monitor { NSEvent.removeMonitor(monitor) }
    }
}

extension NSOutlineView {
    /// Hands `moved` the row AppKit has selected, among `shown`, the rows as the outline shows
    /// them, when they agree in number: SwiftUI tells its selection a moment after AppKit's
    /// changes, so a key pressed straight after a click acted on the row before it.
    func inHand(shown: [String], moved: (String) -> Void) {
        guard numberOfRows == shown.count, selectedRow >= 0, selectedRow < shown.count else { return }
        moved(shown[selectedRow])
    }
}

extension NSView {
    /// The first outline inside this view: AppKit's own, under a SwiftUI `List`.
    var firstOutline: NSOutlineView? {
        (self as? NSOutlineView) ?? subviews.lazy.compactMap(\.firstOutline).first
    }

    /// Hands on the outline inside this view once SwiftUI has made it, looking each 50
    /// milliseconds for up to five seconds: on a slow machine it was not there a turn after the
    /// list appeared, and the list never took a click's focus or a key.
    func whenOutline(_ then: @escaping (NSOutlineView) -> Void, tries: Int = 100) {
        DispatchQueue.main.asyncAfter(deadline: .now() + (tries == 100 ? 0 : 0.05)) { [weak self] in
            guard let self else { return }
            if let outline = self.firstOutline {
                then(outline)
            } else if tries > 0 {
                self.whenOutline(then, tries: tries - 1)
            }
        }
    }
}
