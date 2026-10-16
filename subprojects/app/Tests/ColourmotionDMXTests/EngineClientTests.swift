import XCTest
@testable import ColourmotionDMX

/// The app's own tests cover presentation state only; lighting behaviour is
/// tested in the engine. These exist so `swift test` has something to run and
/// so hard-coded fader counts get caught.
@MainActor
final class EngineClientTests: XCTestCase {
    func testSlotCountFollowsFadersPerPageSetting() {
        let client = EngineClient()
        client.apply(fadersPerPage: 10, legend: "Page 2")
        XCTAssertEqual(client.slots.count, 10)
        XCTAssertEqual(client.pageLegend, "Page 2")
        client.apply(fadersPerPage: 16, legend: "Page 3")
        XCTAssertEqual(client.slots.count, 16)
    }

    func testFaderLevelsAreClampedAndMarkActive() {
        let client = EngineClient()
        client.setFader(index: 0, level: 1.7)
        XCTAssertEqual(client.slots[0].level, 1.0)
        XCTAssertTrue(client.slots[0].isActive)
        client.setFader(index: 0, level: -1)
        XCTAssertEqual(client.slots[0].level, 0.0)
        XCTAssertFalse(client.slots[0].isActive)
    }

    func testOutOfRangeIndexIsIgnored() {
        let client = EngineClient()
        client.setFader(index: 99, level: 0.5)
        XCTAssertTrue(client.slots.allSatisfy { $0.level == 0 })
    }
}
