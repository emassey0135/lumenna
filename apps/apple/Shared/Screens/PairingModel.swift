import Foundation
import LumennaApp
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// Pairing this device with another of the person's: the app's pairing
/// (`ViewModel.devices.pairing`), which runs it and says each step, followed for the pairing
/// screen of the iPhone, the iPad and the Mac.
///
/// On one network the two find each other: both wait, and the system's DNS-SD does the rest.
/// Anywhere else, one shows a code and the other enters it. Either way both show the same three
/// words, and only if the person says they match on both does anything get paired. The words
/// are asked in each platform's own alert (`ask`), whose safe answer, No, is the one a stray
/// Return gives.
final class PairingModel: ObservableObject {
    let core: Core
    /// Every sentence and button, the core's. Another device finds this one on the network by
    /// itself: the system's responder advertises it.
    let words: LumennaApp.PairingWords
    /// What the pairing is doing, in the core's words.
    @Published private(set) var status: String
    /// This device's code, while it waits for the other to enter it.
    @Published private(set) var mine: String?
    /// The other device's code, typed or pasted. An empty one is never filled from the
    /// clipboard: nobody could tell it would, and it sent whatever happened to be copied.
    @Published var entered = ""
    /// A count that moves when the status is worth moving to: a problem, or a code to give.
    @Published private(set) var newsworthy = 0

    /// Asks whether the three words match, and answers.
    var ask: ([String], @escaping (Bool) -> Void) -> Void = { _, answer in answer(false) }
    /// Says what cannot be done, in the core's words.
    var failed: (String) -> Void = { _ in }
    /// Closes the screen once a pairing begun here has ended.
    var finished: () -> Void = {}

    private let name: String
    /// The words asked about, so they are asked once.
    private var askedWords: [String]?
    /// Whether a pairing began here, so its end closes the screen.
    private var began = false
    private var observer: NSObjectProtocol?

    /// `thisDevice` is how the core's sentences name it ("this iPhone"), `name` what the other
    /// device will call it.
    init(core: Core, thisDevice: String, name: String) {
        self.core = core
        self.name = name
        // Pairing's words come with the Devices page, shown here if it is not already.
        if core.app.current.view.devices == nil { core.app.send(.devices(.show)) }
        words = core.app.current.view.devices!.words
        status = words.intro
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Opens the app's pairing, and follows it.
    func open() {
        guard observer == nil else { return }
        if core.app.view.devices == nil { core.app.send(.devices(.show)) }
        core.app.send(.devices(.pair(.open(name))))
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }
    }

    /// Waits for the other device, showing a code it can enter.
    func wait() {
        began = true
        core.app.send(.devices(.pair(.wait)))
    }

    /// Pairs with the code typed or pasted in; an empty field asks for one. A code entered
    /// while this device waits means the person chose the other way: the app gives up the wait
    /// and joins with the code once it has ended.
    func join() {
        let code = entered.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else { return failed(words.needCode) }
        began = true
        core.app.send(.devices(.pair(.code(code))))
        core.app.send(.devices(.pair(.join)))
    }

    /// Copies this device's code, which only Copy Code does: the clipboard is the person's.
    func copyCode() {
        guard let mine else { return }
        #if os(iOS)
        UIPasteboard.general.string = mine
        #else
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(mine, forType: .string)
        #endif
        Announcer.say(words.copied)
    }

    /// Giving up ends the wait for the other device, then the pairing.
    func cancel() {
        began = false
        core.app.send(.devices(.pair(.cancel)))
        core.app.send(.devices(.pair(.cancel)))
    }

    /// The app's pairing, onto the screen: what it is doing, this device's code, the words to
    /// compare; closed once it has paired, which the app says.
    private func follow() {
        guard let pairing = core.app.view.devices?.pairing else {
            if began {
                began = false
                finished()
            }
            return
        }
        let text: String
        if let problem = pairing.problem {
            text = problem
        } else if pairing.mine != nil {
            text = words.waiting
        } else if pairing.running {
            text = pairing.waiting ? words.opening : words.connecting
        } else {
            text = words.intro
        }
        if status != text {
            status = text
            if pairing.problem != nil || pairing.mine != nil { newsworthy += 1 }
        }
        if mine != pairing.mine { mine = pairing.mine }
        if let three = pairing.words, three != askedWords {
            askedWords = three
            ask(three) { [weak self] yes in
                guard let self else { return }
                // What happens next is said while the devices finish, in the core's words.
                self.status = yes ? self.words.finishing : self.words.refusing
                self.core.app.send(.devices(.pair(.confirm(yes))))
            }
        } else if pairing.words == nil {
            askedWords = nil
        }
    }
}
