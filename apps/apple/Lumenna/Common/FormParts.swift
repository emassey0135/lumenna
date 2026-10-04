import SwiftUI

/// The pieces forms here are built from, each because the stock version failed the
/// accessibility audit. Stock pickers pass once the form has SwiftUI's own `.tint`.
enum FormParts {
    /// A section header or footer in grey that still reads: the system's own is under the
    /// contrast text needs.
    static func caption(_ text: String) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(Color(uiColor: .quietLabel))
    }
}

/// A text field's placeholder: an example of what goes there, in a grey that reads. The
/// system placeholder grey is far under contrast, and repeating the field's name beside the
/// name says nothing.
func example(_ text: String) -> Text {
    Text(text).foregroundStyle(Color(uiColor: .quietLabel))
}


/// A text field with its name beside it, read by VoiceOver as one stop: "Due, text field,
/// tomorrow".
///
/// The field carries the name as its label and the visible name is hidden from VoiceOver —
/// how Apple's own UIKit forms do it. The alternatives were each worse: a SwiftUI `TextField`
/// is not named by text beside it, so it read its value with nothing to say which field it
/// was; a visible name that is its own element is a second stop saying the same word; and
/// combining the row loses the text field's role and reads an empty field's placeholder as
/// its value. The accessibility audit flags the hidden name as possibly inaccessible text,
/// so the screens built from these rows are audited with that one finding excused.
struct NamedRow<Control: View>: View {
    let name: String
    @ViewBuilder var control: Control
    @Environment(\.dynamicTypeSize) private var size

    var body: some View {
        // Side by side until the text is large, then the name above the field, so neither
        // is squeezed.
        if size.isAccessibilitySize {
            VStack(alignment: .leading, spacing: 6) {
                label
                field
            }
        } else {
            LabeledContent {
                field
            } label: {
                label
            }
        }
    }

    private var label: some View {
        Text(name).accessibilityHidden(true)
    }

    private var field: some View {
        control.accessibilityLabel(name)
    }
}

/// A button for something hard to take back, in a red that clears contrast — the system's
/// does not on white.
struct WarningButton: View {
    let title: String
    let action: () -> Void

    init(_ title: String, action: @escaping () -> Void) {
        self.title = title
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Text(title).foregroundStyle(Color(uiColor: .warningLabel))
        }
    }
}
