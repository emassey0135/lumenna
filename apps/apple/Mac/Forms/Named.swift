import SwiftUI

/// A form control with its name beside it, read by VoiceOver as one stop: "Repeats, every
/// monday, text".
///
/// In a macOS SwiftUI form a bare `TextField("Title", …)` showed its name as a separate text
/// and the field had no label, so VoiceOver met the name and then a field that did not say
/// what it was. Inside `LabeledContent` the control takes the label as its own name — and
/// adding an accessibility label as well made it "Title, Title".
struct Named<Control: View>: View {
    let name: String
    @ViewBuilder var control: Control

    init(_ name: String, @ViewBuilder control: () -> Control) {
        self.name = name
        self.control = control()
    }

    var body: some View {
        LabeledContent {
            control.labelsHidden()
        } label: {
            Text(name)
        }
    }
}

/// A text field in a form, named as `Named` names it, with an example as its placeholder.
func namedField(_ name: String, text: Binding<String>, example: String = "", axis: Axis = .horizontal) -> some View {
    Named(name) {
        TextField(name, text: text, prompt: example.isEmpty ? nil : Text(example), axis: axis)
    }
}
