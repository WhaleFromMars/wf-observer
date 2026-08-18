import Darwin
import Foundation
import WFObserver

@main
enum WFObserverConsole {
    static func main() async throws {
        guard CommandLine.arguments.count == 2 else {
            FileHandle.standardError.write(
                Data("usage: WFObserverConsole <endpoint-ticket>\n".utf8)
            )
            exit(2)
        }

        let client = try await WFObserver.connect(endpoint: CommandLine.arguments[1])

        do {
            try await client.ping()
        } catch {
            try? await client.shutdown()
            throw error
        }

        try await client.shutdown()
        print("WF Observer ping succeeded")
    }
}
