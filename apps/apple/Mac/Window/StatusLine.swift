import AppKit

/// The main window's content with a status line at its foot: what the app last said, for a person
/// who reads it, as the Windows and GTK apps keep one. VoiceOver has spoken each sentence already
/// (`Announcer`), so the line posts nothing as it changes; VoiceOver can still read it there.
final class StatusLineController: NSViewController {
    private let content: NSViewController
    let line = NSTextField(wrappingLabelWithString: "")
    private var observer: NSObjectProtocol?

    init(content: NSViewController) {
        self.content = content
        super.init(nibName: nil, bundle: nil)
        title = content.title
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    override func loadView() {
        let view = NSView()
        addChild(content)
        let separator = NSBox()
        separator.boxType = .separator
        line.font = .systemFont(ofSize: NSFont.smallSystemFontSize)
        line.textColor = .labelColor
        line.maximumNumberOfLines = 2
        line.setAccessibilityIdentifier("status")
        // Nothing for VoiceOver to land on until something is said: empty, it is a nameless
        // text. Hidden, it keeps its place in the layout.
        line.isHidden = true
        for part in [content.view, separator, line] {
            part.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(part)
        }
        NSLayoutConstraint.activate([
            content.view.topAnchor.constraint(equalTo: view.topAnchor),
            content.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            content.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            separator.topAnchor.constraint(equalTo: content.view.bottomAnchor),
            separator.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            line.topAnchor.constraint(equalTo: separator.bottomAnchor, constant: 4),
            line.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 12),
            line.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -12),
            line.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -5),
            // A line's height even before anything is said, so the window does not jump.
            line.heightAnchor.constraint(greaterThanOrEqualToConstant: NSFont.smallSystemFontSize + 4),
        ])
        self.view = view
        observer = NotificationCenter.default.addObserver(forName: Said.posted, object: nil, queue: .main) { [weak self] note in
            guard let self, let text = Said.text(of: note) else { return }
            self.line.stringValue = text
            self.line.isHidden = false
        }
    }
}
