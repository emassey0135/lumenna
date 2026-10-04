import SwiftUI

/// The pieces both Apple apps' forms are built from. Each exists because the stock version
/// failed the accessibility audit or VoiceOver on one platform or the other, and where the
/// platforms need different answers the difference is here, once, rather than in every form.

// MARK: - Colours

extension Color {
    #if os(iOS)
    static let quietLabel = Color(uiColor: .quietLabel)
    static let lumennaTint = Color(uiColor: .lumennaTint)
    static let warningLabel = Color(uiColor: .warningLabel)
    #else
    static let quietLabel = Color(nsColor: .quietLabel)
    static let lumennaTint = Color(nsColor: .lumennaTint)
    static let warningLabel = Color(nsColor: .warningLabel)
    #endif
}

enum FormParts {
    /// A section header or footer in grey that still reads: the system's own is under the
    /// contrast text needs.
    static func caption(_ text: String) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(Color.quietLabel)
    }

    /// A section's title, which VoiceOver can move between with its heading commands.
    static func heading(_ text: String) -> some View {
        #if os(iOS)
        caption(text).accessibilityAddTraits(.isHeader)
        #else
        Text(text).accessibilityAddTraits(.isHeader)
        #endif
    }
}

/// A text field's placeholder: an example of what goes there, in a grey that reads. The
/// system placeholder grey is far under contrast, and repeating the field's name says nothing.
func example(_ text: String) -> Text {
    Text(text).foregroundStyle(Color.quietLabel)
}

// MARK: - Naming controls

/// A control with its name beside it, read by VoiceOver as one stop: "Due, text field,
/// tomorrow". The platforms need opposite answers:
///
/// - **iOS**: a SwiftUI `TextField` is not named by text beside it, and
///   `accessibilityLabeledPair` does not join them. So the field carries the name as its
///   label and the visible name is hidden — how Apple's UIKit forms do it. Combining the row
///   instead lost the field's role and read an empty field's placeholder as its value.
/// - **macOS**: inside `LabeledContent` the control takes the label as its name by itself;
///   labelling it as well made it "Title, Title". Outside one, a form's field had no name.
struct Named<Control: View>: View {
    let name: String
    @ViewBuilder var control: Control
    #if os(iOS)
    @Environment(\.dynamicTypeSize) private var size
    #endif

    init(_ name: String, @ViewBuilder control: () -> Control) {
        self.name = name
        self.control = control()
    }

    var body: some View {
        #if os(iOS)
        // Side by side until the text is large, then the name above the field, so neither
        // is squeezed.
        if size.isAccessibilitySize {
            VStack(alignment: .leading, spacing: 6) {
                Text(name).accessibilityHidden(true)
                control.accessibilityLabel(name)
            }
        } else {
            LabeledContent {
                control.accessibilityLabel(name)
            } label: {
                Text(name).accessibilityHidden(true)
            }
        }
        #else
        LabeledContent {
            control.labelsHidden()
        } label: {
            Text(name)
        }
        #endif
    }
}

/// A text field in a form, named as `Named` names it, with an example as its placeholder.
func namedField(_ name: String, text: Binding<String>, example placeholder: String = "", axis: Axis = .horizontal) -> some View {
    Named(name) {
        #if os(iOS)
        // An empty title, so the name is the label once rather than twice.
        TextField("", text: text, prompt: placeholder.isEmpty ? nil : example(placeholder), axis: axis)
            .multilineTextAlignment(.trailing)
        #else
        TextField(name, text: text, prompt: placeholder.isEmpty ? nil : Text(placeholder), axis: axis)
        #endif
    }
}

/// A control that names itself on iOS — a toggle, a date picker, a stepper — and needs
/// `Named` to be named in a macOS form.
struct Labelled<Control: View>: View {
    let name: String
    @ViewBuilder var control: Control

    init(_ name: String, @ViewBuilder control: () -> Control) {
        self.name = name
        self.control = control()
    }

    var body: some View {
        #if os(iOS)
        control
        #else
        Named(name) { control }
        #endif
    }
}

/// A choice among a few, as its own section: on iOS a row per choice with the chosen one
/// checked, since a pop-up's value is clipped at large text sizes; on macOS the pop-up menu,
/// as Mac forms have it.
struct ChoiceSection<Value: Hashable>: View {
    let name: String
    @Binding var selection: Value
    let choices: [(label: String, value: Value)]
    var footer: String?

    init(_ name: String, selection: Binding<Value>, choices: [(label: String, value: Value)], footer: String? = nil) {
        self.name = name
        _selection = selection
        self.choices = choices
        self.footer = footer
    }

    var body: some View {
        Section {
            #if os(iOS)
            picker.pickerStyle(.inline).labelsHidden()
            #else
            Named(name) { picker }
            #endif
        } header: {
            #if os(iOS)
            FormParts.heading(name)
            #endif
        } footer: {
            if let footer { FormParts.caption(footer) }
        }
    }

    private var picker: some View {
        Picker(name, selection: $selection) {
            ForEach(choices, id: \.value) { choice in
                Text(choice.label).tag(choice.value)
            }
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
            Text(title).foregroundStyle(Color.warningLabel)
        }
    }
}

/// The failure alert every form shows, from a model's `failure`.
struct FailureAlert: ViewModifier {
    @Binding var failure: String?

    func body(content: Content) -> some View {
        content
            // SwiftUI's own accent, which the window's tint does not reach.
            .tint(Color.lumennaTint)
            .alert(
                "Could not do that",
                isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } }),
                presenting: failure
            ) { _ in
                Button("OK") {}
            } message: { failure in
                Text(failure)
            }
    }
}
