import LumennaApp
import SwiftUI

/// The pairing screen over `PairingModel`, the same on the iPhone, the iPad and the Mac but for
/// what each platform does its own way: the iPhone's filled buttons and the system's Paste
/// button beside the code field, the Mac's named field, Command-V and Cancel.
struct PairingScreen: View {
    @ObservedObject var model: PairingModel
    #if os(iOS)
    @Environment(\.dynamicTypeSize) private var size
    @AccessibilityFocusState private var onStatus: Bool
    #else
    /// Closes the sheet, having given up.
    let close: () -> Void
    #endif

    var body: some View {
        #if os(iOS)
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                status.accessibilityFocused($onStatus)
                filled(model.words.wait) { model.wait() }
                code
                entry
                filled(model.words.join) { model.join() }
            }
            .padding()
        }
        .onChange(of: model.newsworthy) { _, _ in
            // A problem, or a code to give: VoiceOver goes there, once it is drawn.
            DispatchQueue.main.async { onStatus = true }
        }
        #else
        VStack(alignment: .leading, spacing: 12) {
            status
            Button(model.words.wait) { model.wait() }
            code
            Named(model.words.theirCode) {
                // Pasted with Command-V, or typed.
                TextField(model.words.theirCode, text: $model.entered)
                    .onSubmit { model.join() }
            }
            Button(model.words.join) { model.join() }
            HStack {
                Spacer()
                Button("Cancel") {
                    model.cancel()
                    close()
                }
                .keyboardShortcut(.cancelAction)
            }
        }
        .padding(20)
        .frame(width: 480)
        // A problem, or a code to give, said as it comes.
        .onChange(of: model.newsworthy) { _, _ in Announcer.say(model.status) }
        #endif
    }

    private var status: some View {
        Text(model.status).fixedSize(horizontal: false, vertical: true)
    }

    /// This device's code while it waits, and Copy Code, the only way it is copied: the
    /// clipboard is the person's.
    @ViewBuilder private var code: some View {
        if let mine = model.mine {
            Text(mine)
                .font(.body.monospaced())
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
                #if os(iOS)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(uiColor: .secondarySystemBackground), in: RoundedRectangle(cornerRadius: 8))
                #endif
                .accessibilityLabel(model.words.myCode)
                .accessibilityValue(mine)
            #if os(iOS)
            filled(model.words.copyCode) { model.copyCode() }
            #else
            Button(model.words.copyCode) { model.copyCode() }
            #endif
        }
    }

    #if os(iOS)
    /// The field's name above it, then the field and the system's Paste button side by side,
    /// or one above the other at the largest text sizes, where side by side leaves the field
    /// too narrow to read.
    private var entry: some View {
        VStack(alignment: .leading, spacing: 6) {
            // Shown for sight; VoiceOver hears the name on the field.
            Text(model.words.theirCode).accessibilityHidden(true)
            if size.isAccessibilitySize {
                VStack(alignment: .leading, spacing: 8) { field; paste }
            } else {
                HStack(spacing: 8) { field; paste }
            }
        }
    }

    /// Wraps rather than scrolling sideways, as every entry here does: the code is long. An
    /// empty title, so the name is said once, from its label.
    private var field: some View {
        TextField("", text: $model.entered, axis: .vertical)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .textFieldStyle(.roundedBorder)
            .accessibilityLabel(model.words.theirCode)
            .onChange(of: model.entered) { _, text in
                // Return, which a wrapping field takes as a new line, pairs.
                guard text.contains("\n") else { return }
                model.entered = text.replacingOccurrences(of: "\n", with: "")
                model.join()
            }
    }

    /// The system's Paste button: it pastes without the "allow paste" prompt, because the
    /// person pressed it. Nothing here ever reads the clipboard by itself.
    private var paste: some View {
        PasteButton(payloadType: String.self) { strings in
            guard let text = strings.first else { return }
            DispatchQueue.main.async { model.entered = text }
        }
        .labelStyle(.titleAndIcon)
        .buttonBorderShape(.roundedRectangle)
        // The filled buttons' colours, which pass contrast; the system's own may not.
        .tint(Color.lumennaTint)
        .fixedSize()
    }

    private func filled(_ title: String, action: @escaping () -> Void) -> some View {
        Button(title, action: action)
            .buttonStyle(.borderedProminent)
            .tint(Color.lumennaTint)
            .controlSize(.large)
    }
    #endif
}
