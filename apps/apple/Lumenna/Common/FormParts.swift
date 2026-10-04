import SwiftUI

/// The pieces every form here is built from, each chosen because the stock version failed the
/// accessibility audit.
enum FormParts {
    /// A section header or footer in grey that still reads: the system's own is under the
    /// contrast text needs.
    static func caption(_ text: String) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(Color.primary.opacity(0.78))
            // The UI tests know captions by this: the audit reports some as only partly
            // scaling, which screenshots at the largest size show is not so.
            .accessibilityIdentifier("caption")
    }
}

/// An explanation at the foot of a section, as a row of its own. A section's footer stops
/// growing with Dynamic Type part of the way up; a row does not.
struct Note: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        FormParts.caption(text)
    }
}

/// A text field's placeholder: an example of what goes there, in a grey that reads. The
/// system placeholder grey is far under contrast, and repeating the field's name beside the
/// name says nothing.
func example(_ text: String) -> Text {
    Text(text).foregroundStyle(Color(uiColor: .quietLabel))
}

/// One choice among a few, as rows with the chosen one checked — the picker in every form
/// here. The stock inline picker's checkmark failed contrast; this one is drawn in the text
/// colour, and the row says it is selected in words as well as with the mark (§13).
struct ChoiceRows<Value: Hashable>: View {
    let choices: [(String, Value)]
    @Binding var selection: Value

    var body: some View {
        ForEach(choices, id: \.1) { name, value in
            Button {
                selection = value
            } label: {
                HStack {
                    Text(name).foregroundStyle(Color.primary)
                    Spacer()
                    if value == selection {
                        Image(systemName: "checkmark")
                            .foregroundStyle(Color.primary)
                            .accessibilityHidden(true)
                    }
                }
            }
            .accessibilityAddTraits(value == selection ? .isSelected : [])
        }
    }
}

/// A control with its name beside it as ordinary text, the two tied together for VoiceOver.
///
/// Neither half alone works. A SwiftUI control in `LabeledContent` is not named by the label
/// beside it, so it reads its value with nothing to say what it is; hiding the visible name
/// leaves text on screen that no accessibility element covers, which the audit rightly flags.
/// `accessibilityLabeledPair` is the declared relationship between the two.
struct NamedRow<Control: View>: View {
    let name: String
    @ViewBuilder var control: Control
    @Namespace private var pair
    @Environment(\.dynamicTypeSize) private var size

    var body: some View {
        // Side by side until the text is large, then the name above the control, so neither
        // is squeezed.
        if size.isAccessibilitySize {
            VStack(alignment: .leading, spacing: 6) {
                label
                content
            }
        } else {
            LabeledContent {
                content
            } label: {
                label
            }
        }
    }

    private var label: some View {
        Text(name)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityLabeledPair(role: .label, id: name, in: pair)
    }

    private var content: some View {
        control
            .accessibilityLabel(name)
            .accessibilityLabeledPair(role: .content, id: name, in: pair)
    }
}

/// A date or time with its name beside it. A `DatePicker`'s own label stops growing with
/// Dynamic Type; this one does not.
struct DateRow: View {
    let name: String
    @Binding var selection: Date
    var components: DatePickerComponents = .hourAndMinute

    var body: some View {
        NamedRow(name: name) {
            // An empty title, so the name is the label once rather than twice.
            DatePicker("", selection: $selection, displayedComponents: components)
                .labelsHidden()
        }
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
