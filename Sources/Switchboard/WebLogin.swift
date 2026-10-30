import AppKit
import AuthenticationServices
import CryptoKit
import Network

@MainActor
protocol WebLoginProvider {
    func signIn(window: NSWindow) async throws -> TargetSnapshot
    func cancel()
}

/// 与 Pen 桌面端相同的 PKCE + 本机回调；认证窗口使用独立 Cookie 会话。
@MainActor
final class PenWebLogin: NSObject, WebLoginProvider, ASWebAuthenticationPresentationContextProviding {
    private var listener: NWListener?
    private var session: ASWebAuthenticationSession?
    private weak var window: NSWindow?
    private var pending: CheckedContinuation<String, Error>?
    private var expectedState = ""
    private var receivedCode = false

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        window ?? ASPresentationAnchor()
    }

    private static func pkceVerifier() -> String {
        var rng = SystemRandomNumberGenerator()
        var bytes = [UInt8](repeating: 0, count: 32)
        for index in bytes.indices {
            bytes[index] = UInt8.random(in: 0...255, using: &rng)
        }
        return Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    static func error(_ text: String) -> NSError {
        NSError(domain: "Switchboard.WebLogin", code: 1, userInfo: [NSLocalizedDescriptionKey: text])
    }

    static func callbackCode(_ request: String, state: String) -> String? {
        let parts = request.components(separatedBy: "\r\n")[0].split(separator: " ")
        guard parts.count == 3, parts[0] == "GET",
              let url = URLComponents(string: "http://127.0.0.1\(parts[1])"), url.path == "/callback" else { return nil }
        let items = url.queryItems ?? []
        guard items.filter({ $0.name == "state" }).count == 1,
              items.first(where: { $0.name == "state" })?.value == state,
              items.filter({ $0.name == "code" }).count == 1,
              let code = items.first(where: { $0.name == "code" })?.value, !code.isEmpty else { return nil }
        return code
    }

    private func accept(_ connection: NWConnection) {
        connection.start(queue: .main)
        read(connection, buffer: Data())
    }

    private func read(_ connection: NWConnection, buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 8192) { [weak self] data, _, complete, error in
            Task { @MainActor in
                guard let self else { connection.cancel(); return }
                var buffer = buffer
                if let data { buffer.append(data) }
                guard buffer.count <= 16384, error == nil else { connection.cancel(); return }
                let text = String(decoding: buffer, as: UTF8.self)
                guard text.contains("\r\n\r\n") else {
                    if complete { connection.cancel() } else { self.read(connection, buffer: buffer) }
                    return
                }
                let code = Self.callbackCode(text, state: self.expectedState)
                let valid = code != nil && !self.receivedCode && self.pending != nil
                let body = valid ? "Login received. You can return to Pencil Account Switcher." : "Invalid or expired login callback."
                let reply = "HTTP/1.1 \(valid ? "200 OK" : "400 Bad Request")\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: \(body.utf8.count)\r\n\r\n\(body)"
                connection.send(content: Data(reply.utf8), completion: .contentProcessed { _ in connection.cancel() })
                if valid {
                    self.receivedCode = true
                    let pending = self.pending
                    self.pending = nil
                    pending?.resume(returning: code!)
                }
            }
        }
    }

    func signIn(window: NSWindow) async throws -> TargetSnapshot {
        self.window = window
        receivedCode = false
        expectedState = UUID().uuidString
        let verifier = Self.pkceVerifier()
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let server = try NWListener(using: parameters)
        listener = server
        defer {
            pending = nil
            server.cancel()
            listener = nil
            session?.cancel()
            session = nil
        }
        server.newConnectionHandler = { [weak self] connection in Task { @MainActor in self?.accept(connection) } }
        try await withCheckedThrowingContinuation { (ready: CheckedContinuation<Void, Error>) in
            var settled = false
            server.stateUpdateHandler = { state in
                Task { @MainActor in
                guard !settled else { return }
                switch state {
                case .ready: settled = true; ready.resume()
                case .failed(let error): settled = true; ready.resume(throwing: error)
                case .cancelled: settled = true; ready.resume(throwing: CancellationError())
                default: break
                }
                }
            }
            server.start(queue: .main)
        }
        try Task.checkCancellation()
        let device = PenQuotaProvider.deviceId()
        guard let port = server.port else { throw Self.error("无法打开本机登录回调") }
        let start = try await post("start", body: ["codeChallenge": challenge,
            "redirectUri": "http://127.0.0.1:\(port.rawValue)/callback", "deviceId": device,
            "clientType": "desktop", "clientHost": "darwin"])
        guard let path = start["loginUrl"] as? String,
              let url = URL(string: path, relativeTo: URL(string: "https://pen.dev")!)?.absoluteURL,
              url.scheme == "https", ["pen.dev", "www.pen.dev"].contains(url.host ?? ""),
              var components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            throw Self.error("登录接口返回了无效的网址")
        }
        components.queryItems = (components.queryItems ?? []).filter { $0.name != "state" } + [URLQueryItem(name: "state", value: expectedState)]
        let code = try await withTaskCancellationHandler(operation: {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<String, Error>) in
                pending = continuation
                let auth = ASWebAuthenticationSession(url: components.url!, callbackURLScheme: nil) { [weak self] _, error in
                    Task { @MainActor in
                        if let error = error as? ASWebAuthenticationSessionError, error.code == .canceledLogin {
                            self?.finish(CancellationError())
                        } else if let error { self?.finish(error) }
                    }
                }
                auth.prefersEphemeralWebBrowserSession = true
                auth.presentationContextProvider = self
                session = auth
                if !auth.start() { finish(Self.error("无法打开系统认证窗口")) }
            }
        }, onCancel: { Task { @MainActor in self.cancel() } })
        try Task.checkCancellation()
        let result = try await post("token", body: ["code": code, "codeVerifier": verifier, "deviceId": device, "clientType": "desktop"])
        guard let email = result["email"] as? String, email.contains("@"),
              let token = result["sessionToken"] as? String, !token.isEmpty else {
            throw Self.error("登录结果缺少账号信息")
        }
        guard result["needsCompletion"] as? Bool != true else { throw Self.error("请先在 Pen 网页完成账号资料，再重新添加") }
        var sessionObject: [String: Any] = ["email": email, "token": token]
        let skipped = Set(["email", "sessionToken", "needsCompletion"])
        for (key, value) in result where !skipped.contains(key) && (value is String || value is NSNumber) {
            sessionObject[key] = value
        }
        let data = try JSONSerialization.data(withJSONObject: sessionObject)
        return TargetSnapshot(sessionFile: String(decoding: data, as: UTF8.self), email: email, token: token)
    }

    private func post(_ path: String, body: [String: String]) async throws -> [String: Any] {
        var request = URLRequest(url: URL(string: "https://api.pencil.dev/auth/device-auth/\(path)")!)
        request.httpMethod = "POST"
        request.timeoutInterval = 30
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Self.error("网页登录接口请求失败，请稍后重试")
        }
        return json
    }

    private func finish(_ error: Error) {
        let continuation = pending
        pending = nil
        continuation?.resume(throwing: error)
    }

    func cancel() {
        finish(CancellationError())
        session?.cancel()
        listener?.cancel()
    }
}
