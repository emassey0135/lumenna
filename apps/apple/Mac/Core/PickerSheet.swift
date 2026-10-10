import AppKit

/// One thing to choose: what identifies it to the core, and how it reads.
struct PickerItem {
    var key: String
    var title: String
    var detail: String?
    /// How deep it sits in a tree, shown as indentation, as the Mac's outlines show it.
    var depth = 0
}

/// Choosing one of many — a task to wait for, a project to move under, a block to put a task
/// in — as a sheet: a field to narrow the list, the list, and Choose.
///
/// A table rather than a menu, because the list can be long; the field narrows it as typed
/// and the arrow keys move through the list without leaving the field, as Spotlight's do.
final class PickerSheet: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {
    private let titleText: String
    private let items: [PickerItem]
    private var shown: [PickerItem]
    private let chosen: (PickerItem) -> Void
    private let table = NSTableView()
    private let search = NSSearchField()
    private let count = NSTextField(labelWithString: "")

    private init(title: String, items: [PickerItem], chosen: @escaping (PickerItem) -> Void) {
        titleText = title
        self.items = items
        shown = items
        self.chosen = chosen
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// Shows the sheet on `window`. An empty list says so instead.
    static func present(on window: NSWindow, title: String, items: [PickerItem], chosen: @escaping (PickerItem) -> Void) {
        guard !items.isEmpty else {
            window.showFailure("There is nothing to choose from.", title: title)
            return
        }
        let sheet = PickerSheet(title: title, items: items, chosen: chosen)
        let host = NSWindow(contentViewController: sheet)
        host.title = title
        window.beginSheet(host)
    }

    override func loadView() {
        let heading = NSTextField(labelWithString: titleText)
        heading.font = .preferredFont(forTextStyle: .headline)
        heading.setAccessibilityRole(.staticText)

        search.placeholderString = "Narrow the list"
        search.setAccessibilityLabel("Narrow \(titleText)")
        search.delegate = self
        search.sendsSearchStringImmediately = true

        count.textColor = .quietLabel

        let column = NSTableColumn(identifier: .init("item"))
        column.title = titleText
        table.addTableColumn(column)
        table.headerView = nil
        table.dataSource = self
        table.delegate = self
        table.usesAutomaticRowHeights = true
        table.doubleAction = #selector(choose)
        table.target = self
        table.setAccessibilityLabel(titleText)
        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder

        let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        cancel.keyEquivalent = "\u{1b}"
        let choose = NSButton(title: "Choose", target: self, action: #selector(choose))
        choose.keyEquivalent = "\r"
        let buttons = NSStackView(views: [NSView(), cancel, choose])
        buttons.distribution = .fill

        let stack = NSStackView(views: [heading, search, count, scroll, buttons])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        for view in [search, scroll, buttons] {
            view.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -32).isActive = true
        }
        scroll.heightAnchor.constraint(equalToConstant: 280).isActive = true
        stack.widthAnchor.constraint(equalToConstant: 440).isActive = true
        view = stack
        update()
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(search)
    }

    private func update() {
        let text = search.stringValue.trimmingCharacters(in: .whitespaces).lowercased()
        shown = text.isEmpty ? items : items.filter {
            $0.title.lowercased().contains(text) || ($0.detail?.lowercased().contains(text) ?? false)
        }
        count.stringValue = shown.count == 1 ? "1 choice" : "\(shown.count) choices"
        table.reloadData()
        if !shown.isEmpty {
            table.selectRowIndexes([0], byExtendingSelection: false)
        }
    }

    func controlTextDidChange(_ notification: Notification) {
        update()
    }

    /// Up and down in the field move through the list, so narrowing and choosing need no
    /// trip between the two.
    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        let row = table.selectedRow
        switch selector {
        case #selector(NSResponder.moveDown(_:)) where row + 1 < shown.count:
            select(row + 1)
        case #selector(NSResponder.moveUp(_:)) where row > 0:
            select(row - 1)
        case #selector(NSResponder.insertNewline(_:)):
            choose()
        case #selector(NSResponder.cancelOperation(_:)):
            cancel()
        default:
            return false
        }
        return true
    }

    private func select(_ row: Int) {
        table.selectRowIndexes([row], byExtendingSelection: false)
        table.scrollRowToVisible(row)
        // The field keeps focus, so say what is now chosen.
        let item = shown[row]
        Announcer.say([item.title, item.detail].compactMap { $0 }.joined(separator: ", "))
    }

    func numberOfRows(in tableView: NSTableView) -> Int { shown.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let item = shown[row]
        let cell = TwoLineCell()
        cell.show(title: item.title, detail: item.detail)
        cell.indent = CGFloat(item.depth) * 16
        return cell
    }

    @objc private func choose() {
        let row = table.selectedRow
        guard row >= 0, row < shown.count else { return }
        let item = shown[row]
        let chosen = self.chosen
        close { chosen(item) }
    }

    @objc private func cancel() {
        close {}
    }

    private func close(then: @escaping () -> Void) {
        guard let sheet = view.window, let parent = sheet.sheetParent else { return }
        parent.endSheet(sheet)
        DispatchQueue.main.async(execute: then)
    }
}

/// A title with a quieter line beneath it — the shape of nearly every row in the app.
final class TwoLineCell: NSTableCellView {
    let title = NSTextField(labelWithString: "")
    let detail = NSTextField(labelWithString: "")

    override init(frame: NSRect) {
        super.init(frame: frame)
        title.lineBreakMode = .byWordWrapping
        title.maximumNumberOfLines = 0
        title.cell?.wraps = true
        detail.font = .preferredFont(forTextStyle: .subheadline)
        detail.textColor = .quietLabel
        detail.lineBreakMode = .byWordWrapping
        detail.maximumNumberOfLines = 0
        detail.cell?.wraps = true
        let stack = NSStackView(views: [title, detail])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 1
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        leading = stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2)
        NSLayoutConstraint.activate([
            leading,
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 3),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -3),
        ])
        textField = title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    private var warning = false
    private var leading: NSLayoutConstraint!

    /// Indentation for a row's depth, for sight: VoiceOver is not told it.
    var indent: CGFloat = 0 {
        didSet { leading.constant = 2 + indent }
    }

    func show(title text: String, detail more: String?, warning: Bool = false) {
        title.stringValue = text
        detail.stringValue = more ?? ""
        detail.isHidden = (more ?? "").isEmpty
        self.warning = warning
        colour()
    }

    /// On a selected row the highlight is the background, so both lines take the colour the
    /// system gives selected text: a quiet grey or red there falls under contrast.
    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { colour() }
    }

    private func colour() {
        if backgroundStyle == .emphasized {
            title.textColor = .alternateSelectedControlTextColor
            detail.textColor = .alternateSelectedControlTextColor
        } else {
            title.textColor = .labelColor
            detail.textColor = warning ? .warningLabel : .quietLabel
        }
    }
}
