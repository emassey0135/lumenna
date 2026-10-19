import LumennaApp
import SwiftUI

/// The Mac's pairing screen over `PairingModel`, the iPhone's too: what the pairing is doing,
/// Wait for the Other Device, this Mac's code and Copy Code while it waits, the other device's
/// code (pasted with Command-V, or typed) and Pair Using This Code, and Cancel.
struct PairingScreen: View {
    @ObservedObject var model: PairingModel
    /// Closes the sheet, having given up.
    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(model.status).fixedSize(horizontal: false, vertical: true)
            Button(model.words.wait) { model.wait() }
            if let mine = model.mine {
                Text(mine)
                    .font(.body.monospaced())
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityLabel(model.words.myCode)
                    .accessibilityValue(mine)
                // The only way the code is copied: the clipboard is the person's.
                Button(model.words.copyCode) { model.copyCode() }
            }
            Named(model.words.theirCode) {
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
    }
}
