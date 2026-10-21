import UIKit
import XCTest
@testable import Lumenna

/// What the app says is shown as well as spoken, for someone who reads it: a banner over the
/// window, which VoiceOver skips, since it has said it already, and which takes no touches.
@MainActor
final class SaidBannerTests: XCTestCase {
    private func settle(_ until: () -> Bool) {
        let deadline = Date().addingTimeInterval(5)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !until() && Date() < deadline
    }

    func testWhatIsSaidIsShownOnTheWindowAndLeftToVoiceOverToSay() {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        let banner = SaidBanner()
        banner.place(on: window)
        XCTAssertNil(banner.shown)

        Announcer.say("Unpaired Kitchen iPad")
        settle { banner.shown != nil }

        XCTAssertEqual(banner.shown, "Unpaired Kitchen iPad")
        XCTAssertTrue(banner.accessibilityElementsHidden, "VoiceOver has said it already")
        XCTAssertFalse(banner.isUserInteractionEnabled, "a tap goes to what is under it")
        XCTAssertFalse(window.hitTest(banner.center, with: nil) === banner)

        Announcer.say("Archived Home")
        settle { banner.shown == "Archived Home" }
        XCTAssertEqual(banner.shown, "Archived Home", "a later sentence takes its place")
    }

    func testUnderTheUITestsNoBannerIsPutOnTheWindow() {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        XCTAssertNil(SaidBanner.install(on: window))
        XCTAssertFalse(window.subviews.contains { $0 is SaidBanner })
    }
}
