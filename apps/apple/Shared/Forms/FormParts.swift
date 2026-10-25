import LumennaApp
import SwiftUI

/// The pieces the Apple apps' forms are built from: the iPhone's, the Mac's and the watch's.
/// Each exists because the stock version failed the accessibility audit or VoiceOver on one
/// platform or another, and where the platforms need different answers the difference is
/// here, once, rather than in every form. The watch takes the iPhone's answer wherever its
/// SwiftUI behaves as the iPhone's does.

enum FormParts {
    /// A section header or footer in grey that still reads: the system's own is under the
    /// contrast text needs.
    static func caption(_ text: String) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(Color.quietLabel)
            // Wraps as it grows, never truncated: a line that fits now is one a larger text
            // size would clip.
            .fixedSize(horizontal: false, vertical: true)
    }

    /// A section's title, which VoiceOver can move between with its heading commands.
    static func heading(_ text: String) -> some View {
        #if os(iOS) || os(watchOS)
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
/// - **watchOS**: as iOS, with the name always above the field: a watch is too narrow for
///   both on one line.
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
        // Side by side where the name fits whole, else the name above the field, so neither
        // is squeezed: at the accessibility sizes always, and on a narrow phone sooner.
        let stacked = VStack(alignment: .leading, spacing: 6) {
            Text(name).accessibilityHidden(true)
            control.accessibilityLabel(name)
        }
        if size.isAccessibilitySize {
            stacked
        } else {
            ViewThatFits(in: .horizontal) {
                LabeledContent {
                    control.accessibilityLabel(name)
                } label: {
                    Text(name).fixedSize().accessibilityHidden(true)
                }
                stacked
            }
        }
        #elseif os(watchOS)
        VStack(alignment: .leading, spacing: 4) {
            Text(name).font(.footnote).foregroundStyle(Color.quietLabel).accessibilityHidden(true)
            control.accessibilityLabel(name)
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
        #elseif os(watchOS)
        TextField("", text: text, prompt: placeholder.isEmpty ? nil : example(placeholder), axis: axis)
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
        #if os(iOS) || os(watchOS)
        control
        #else
        Named(name) { control }
        #endif
    }
}

/// A date or time picker, named as `Named` names a field. Left to SwiftUI, the iPhone drew
/// its title as text beside a picker VoiceOver read unnamed ("Time Picker"), and the watch
/// showed and said no name at all ("Hour, 9, Minute, 00, AM"). The title is empty where the
/// picker carries the name, so the name is said once; on the watch the name is the text above.
struct NamedDatePicker: View {
    let name: String
    @Binding var selection: Date
    let components: DatePickerComponents
    /// Said after the name, on the picker itself.
    let hint: String

    init(_ name: String, selection: Binding<Date>, displayedComponents components: DatePickerComponents, hint: String = "") {
        self.name = name
        _selection = selection
        self.components = components
        self.hint = hint
    }

    var body: some View {
        #if os(watchOS)
        // The name shown and read above the picker, which is left its own parts' names: a
        // label on it went to each part, so Month, Day and Year were all read as the name.
        VStack(alignment: .leading, spacing: 4) {
            Text(name).font(.footnote).foregroundStyle(Color.quietLabel)
            DatePicker("", selection: $selection, displayedComponents: components)
                .labelsHidden()
                .modifier(FieldHint(hint))
        }
        #else
        Named(name) {
            #if os(macOS)
            DatePicker(name, selection: $selection, displayedComponents: components)
                .modifier(FieldHint(hint))
            #else
            DatePicker("", selection: $selection, displayedComponents: components)
                .labelsHidden()
                .modifier(FieldHint(hint))
            #endif
        }
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
            #if os(iOS) || os(watchOS)
            picker.pickerStyle(.inline).labelsHidden()
            #else
            Named(name) { picker }
            #endif
        } header: {
            #if os(iOS) || os(watchOS)
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

// MARK: - The forms' words

/// A form's fields as the core words them (`DetailsView.form`): each field's name, what it
/// takes and an example, the same in every app. The controls are the platform's.
struct FormWords {
    private let fields: [String: LumennaApp.FormField]

    init(_ fields: [LumennaApp.FormField]) {
        self.fields = Dictionary(fields.map { ($0.key, $0) }, uniquingKeysWith: { first, _ in first })
    }

    /// The field `key`; one the core does not describe reads as its key, so a mistake shows.
    subscript(_ key: String) -> LumennaApp.FormField {
        fields[key] ?? LumennaApp.FormField(key: key, label: key, hint: "", example: "", kind: .line, options: [], repeatingOnly: false, syntax: nil, oneDay: false)
    }
}

/// A text field for one of the core's form fields: its name, its example as the placeholder,
/// and what it takes as its hint.
func namedField(_ field: LumennaApp.FormField, text: Binding<String>, axis: Axis = .horizontal) -> some View {
    namedField(field.label, text: text, example: field.example, axis: axis).modifier(FieldHint(field.hint))
}

/// What a field takes, said by VoiceOver after a pause on iOS and the watch, and shown as a
/// tooltip on macOS. Nothing for a field with nothing to say.
struct FieldHint: ViewModifier {
    let text: String

    init(_ text: String) { self.text = text }

    func body(content: Content) -> some View {
        if text.isEmpty {
            content
        } else {
            #if os(iOS) || os(watchOS)
            content.accessibilityHint(text)
            #else
            content.help(text)
            #endif
        }
    }
}

#if os(macOS)
/// A stepper telling VoiceOver what its number stands for, in words: the block's length ("1
/// hour, 5 minutes"), the backups kept ("10"). AppKit's says its number as a percentage of its
/// range ("9.1%"), SwiftUI's said the same before its name and value together, and SwiftUI's own
/// accessibility value on either is ignored; the text beside it is not read again when it
/// changes. Named here, as `Named` cannot name an AppKit control.
struct SpokenStepper: NSViewRepresentable {
    @Binding var value: Int
    let range: ClosedRange<Int>
    let step: Int
    let name: String
    let spoken: (Int) -> String

    init(value: Binding<Int>, in range: ClosedRange<Int>, step: Int = 1, name: String, spoken: @escaping (Int) -> String) {
        _value = value
        self.range = range
        self.step = step
        self.name = name
        self.spoken = spoken
    }

    func makeNSView(context: Context) -> Spoken {
        let stepper = Spoken()
        stepper.minValue = Double(range.lowerBound)
        stepper.maxValue = Double(range.upperBound)
        stepper.increment = Double(step)
        stepper.valueWraps = false
        stepper.target = context.coordinator
        stepper.action = #selector(Coordinator.stepped(_:))
        stepper.setAccessibilityLabel(name)
        return stepper
    }

    func updateNSView(_ stepper: Spoken, context: Context) {
        context.coordinator.value = $value
        stepper.integerValue = value
        stepper.spoken = spoken(value)
    }

    func makeCoordinator() -> Coordinator { Coordinator(value: $value) }

    final class Coordinator: NSObject {
        var value: Binding<Int>
        init(value: Binding<Int>) { self.value = value }
        @objc func stepped(_ stepper: NSStepper) { value.wrappedValue = stepper.integerValue }
    }

    final class Spoken: NSStepper {
        var spoken = "" {
            didSet { if spoken != oldValue { NSAccessibility.post(element: self, notification: .valueChanged) } }
        }

        override func accessibilityValue() -> Any? { spoken }
    }
}
#endif
