import Foundation

public enum BridgeError: Error, Equatable {
    case commandUnavailable
    case invalidResponse
    case remote(String)
    case unavailable
}

@MainActor
public protocol DesktopTransport {
    func send(command: String, argumentsJSON: String) async throws -> String
}

/// All requests pass through one edition gate before reaching the transport.
@MainActor
public struct DesktopBridge {
    public let edition: DesktopEdition
    private let transport: any DesktopTransport

    public init(edition: DesktopEdition, transport: any DesktopTransport) {
        self.edition = edition
        self.transport = transport
    }

    public func invoke(_ command: DesktopCommand, arguments: [String: JSONValue] = [:]) async throws -> JSONValue {
        guard command.isAvailable(in: edition) else { throw BridgeError.commandUnavailable }
        let data = try JSONEncoder().encode(arguments)
        guard let json = String(data: data, encoding: .utf8) else { throw BridgeError.unavailable }
        let response = try await transport.send(command: command.rawValue, argumentsJSON: json)
        guard let responseData = response.data(using: .utf8),
              let envelope = try? JSONDecoder().decode(Envelope.self, from: responseData)
        else { throw BridgeError.invalidResponse }
        if envelope.ok { return envelope.value ?? .null }
        throw BridgeError.remote(envelope.error ?? "操作未完成")
    }

    private struct Envelope: Decodable {
        let ok: Bool
        let value: JSONValue?
        let error: String?

        private enum CodingKeys: String, CodingKey { case ok, value, error }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            ok = try container.decode(Bool.self, forKey: .ok)
            if ok {
                guard container.contains(.value) else {
                    throw DecodingError.keyNotFound(CodingKeys.value, .init(codingPath: container.codingPath, debugDescription: "Missing result"))
                }
                value = try container.decode(JSONValue.self, forKey: .value)
                error = nil
            } else {
                value = nil
                error = try container.decode(String.self, forKey: .error)
            }
        }
    }
}
