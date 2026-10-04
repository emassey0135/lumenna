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

    override func viewDidAppear() {
        super.viewDidAppear()
        // The form's scroll view exists once SwiftUI has laid out.
        if let scroll = firstScrollView(in: host) {
            scroll.setAccessibilityLabel(name)
        } else {
            host.flattened = false
            host.setAccessibilityLabel(name)
        }
    }
}

/// A hosting view that is not itself an accessibility element, so its contents sit directly
/// in whatever holds it.
final class FlatHostingView<Content: View>: NSHostingView<Content> {
    var flattened = true

    override func isAccessibilityElement() -> Bool {
        flattened ? false : super.isAccessibilityElement()
    }
}

private func firstScrollView(in view: NSView) -> NSScrollView? {
    for subview in view.subviews {
        if let scroll = subview as? NSScrollView { return scroll }
        if let found = firstScrollView(in: subview) { return found }
    }
    return nil
}
