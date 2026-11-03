import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

@main
struct UpdateLogicTests {
    static var failures = 0
    static var checks = 0

    static func check(_ condition: Bool, _ label: String) {
        checks += 1
        if !condition {
            failures += 1
            fputs("FAIL \(label)\n", stderr)
        }
        print("\(condition ? "PASS" : "FAIL") \(label)")
    }

    static func main() async {
        await run()
        print("\(checks - failures)/\(checks) checks passed")
        exit(failures == 0 ? 0 : 1)
    }

    static func run() async {
        check(UpdateVersion.zipAsset(from: ["notes.txt", "Pencil-Account-Switcher-1.1.0-arm64.zip"])
                == "Pencil-Account-Switcher-1.1.0-arm64.zip",
              "arm64 zip is preferred")
        check(UpdateVersion.zipAsset(from: [
            "Pencil-Account-Switcher-1.1.0.zip",
            "Pencil-Account-Switcher-1.1.0-arm64.dmg",
        ]) == nil, "non-arm64 zip is rejected")
        check(UpdateVersion.zipAsset(from: [
            "Pencil-Account-Switcher-1.1.0.zip",
            "Pencil-Account-Switcher-1.1.0-arm64.zip",
        ]) == "Pencil-Account-Switcher-1.1.0-arm64.zip",
              "arm64 zip wins over a generic zip")

        let response = HTTPURLResponse(url: URL(string: "https://example.test/app.zip")!,
                                       statusCode: 404, httpVersion: nil, headerFields: nil)
        let httpError = DownloadTask.errorIfUnsuccessful(response) as NSError?
        check(httpError?.localizedDescription.contains("HTTP 404") == true,
              "non-2xx download error includes the status code")
        check(DownloadTask.errorIfUnsuccessful(nil) != nil, "missing HTTP response is rejected")

        let root = FileManager.default.temporaryDirectory.appendingPathComponent("logic-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try! FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let empty = root.appendingPathComponent("empty", isDirectory: true)
        let one = root.appendingPathComponent("one", isDirectory: true)
        let two = root.appendingPathComponent("two", isDirectory: true)
        try! FileManager.default.createDirectory(at: empty, withIntermediateDirectories: true)
        try! FileManager.default.createDirectory(at: one, withIntermediateDirectories: true)
        try! FileManager.default.createDirectory(at: two, withIntermediateDirectories: true)
        try! FileManager.default.createDirectory(at: one.appendingPathComponent("Pencil Account Switcher.app"), withIntermediateDirectories: true)
        try! FileManager.default.createDirectory(at: two.appendingPathComponent("A.app"), withIntermediateDirectories: true)
        try! FileManager.default.createDirectory(at: two.appendingPathComponent("B.app"), withIntermediateDirectories: true)
        do {
            _ = try FileUpdateInstaller.uniqueApp(in: empty)
            check(false, "a package without an app is rejected")
        } catch {
            check(error.localizedDescription.contains("结构异常"), "a package without an app is rejected")
        }
        do {
            let app = try FileUpdateInstaller.uniqueApp(in: one)
            check(app.lastPathComponent == "Pencil Account Switcher.app", "a package with one app is accepted")
        } catch {
            check(false, "a package with one app is accepted")
        }
        do {
            _ = try FileUpdateInstaller.uniqueApp(in: two)
            check(false, "a package with two apps is rejected")
        } catch {
            check(error.localizedDescription.contains("结构异常"), "a package with two apps is rejected")
        }

        await checkDownload(status: 404, body: Data("missing".utf8), delay: 0) { url in
            let task = DownloadTask()
            do {
                let file = try await task.download(url)
                try? FileManager.default.removeItem(at: file)
                check(false, "HTTP 404 download fails with the status code")
            } catch {
                check(error.localizedDescription.contains("HTTP 404"),
                      "HTTP 404 download fails with the status code")
            }
        }

        let payload = Data("zip-bytes".utf8)
        await checkDownload(status: 200, body: payload, delay: 0) { url in
            let task = DownloadTask()
            do {
                let file = try await task.download(url)
                let saved = try Data(contentsOf: file)
                try? FileManager.default.removeItem(at: file)
                check(saved == payload, "HTTP 200 download keeps the body")
            } catch {
                check(false, "HTTP 200 download keeps the body")
            }
        }

        let started = Date()
        await checkDownload(status: 200, body: Data("slow".utf8), delay: 3) { url in
            let task = Task {
                try await DownloadTask().download(url)
            }
            try? await Task.sleep(nanoseconds: 200_000_000)
            task.cancel()
            do {
                let file = try await task.value
                try? FileManager.default.removeItem(at: file)
                check(false, "cancelling a download stops it")
            } catch is CancellationError {
                check(Date().timeIntervalSince(started) < 2, "cancelling a download stops it")
            } catch {
                check(false, "cancelling a download stops it")
            }
        }

        let releaseJSON = """
        {"tag_name":"v1.4.0","html_url":"https://example.test/rel","prerelease":false,"assets":[
          {"name":"Pencil-Account-Switcher-1.4.0.zip","browser_download_url":"https://example.test/any.zip"},
          {"name":"Pencil-Account-Switcher-1.4.0-arm64.zip","browser_download_url":"https://example.test/arm.zip"},
          {"name":"Pencil-Account-Switcher-1.4.0-arm64.zip.sig","browser_download_url":"https://example.test/arm.zip.sig"}
        ]}
        """.data(using: .utf8)!
        await checkDownload(status: 200, body: releaseJSON, delay: 0) { url in
            let feed = GitHubReleaseFeed(repo: "gaogep/pencil-account-switcher", apiBase: url)
            do {
                let release = try await feed.fetchLatest()
                check(release.version == "1.4.0"
                      && release.zipURL.absoluteString == "https://example.test/arm.zip"
                      && release.signatureURL?.absoluteString == "https://example.test/arm.zip.sig",
                      "latest release requires the arm64 zip and its signature")
            } catch {
                check(false, "latest release requires the arm64 zip and its signature")
            }
        }

        let unsignedJSON = """
        {"tag_name":"v1.4.0","html_url":"https://example.test/rel","prerelease":false,"assets":[
          {"name":"Pencil-Account-Switcher-1.4.0-arm64.zip","browser_download_url":"https://example.test/arm.zip"}
        ]}
        """.data(using: .utf8)!
        await checkDownload(status: 200, body: unsignedJSON, delay: 0) { url in
            let feed = GitHubReleaseFeed(repo: "gaogep/pencil-account-switcher", apiBase: url)
            do {
                _ = try await feed.fetchLatest()
                check(false, "a release without a signature asset is rejected")
            } catch {
                check(true, "a release without a signature asset is rejected")
            }
        }
    }

    static func checkDownload(status: Int, body: Data, delay: Double,
                              _ bodyFn: (URL) async throws -> Void) async {
        do {
            let server = try OneShotServer(status: status, body: body, delay: delay)
            let url = URL(string: "http://127.0.0.1:\(server.port)/file")!
            try await bodyFn(url)
            server.stop()
        } catch {
            check(false, "local update server failed: \(error.localizedDescription)")
        }
    }
}

final class OneShotServer {
    let process: Process
    let port: Int

    init(status: Int, body: Data, delay: Double) throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("sb-http-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let script = directory.appendingPathComponent("server.py")
        let payload = directory.appendingPathComponent("body")
        try body.write(to: payload)
        let source = """
        import socket, sys, time
        status = int(sys.argv[1])
        delay = float(sys.argv[3])
        body = open(sys.argv[2], "rb").read()
        server = socket.socket()
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        print(server.getsockname()[1], flush=True)
        conn, _ = server.accept()
        time.sleep(delay)
        data = b""
        while b"\\r\\n\\r\\n" not in data:
            chunk = conn.recv(65536)
            if not chunk:
                break
            data += chunk
        head = (
            f"HTTP/1.1 {status} {'OK' if status < 400 else 'ERR'}\\r\\n"
            f"Content-Length: {len(body)}\\r\\n"
            "Connection: close\\r\\n\\r\\n"
        ).encode()
        try:
            conn.sendall(head + body)
        except Exception:
            pass
        conn.close()
        server.close()
        """
        try source.write(to: script, atomically: true, encoding: .utf8)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["python3", script.path, String(status), payload.path, String(delay)]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice
        try process.run()
        var data = Data()
        while !data.contains(0x0A) {
            let chunk = pipe.fileHandleForReading.readData(ofLength: 1)
            if chunk.isEmpty { break }
            data.append(chunk)
        }
        guard let line = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              let port = Int(line) else {
            process.terminate()
            throw URLError(.badServerResponse)
        }
        self.process = process
        self.port = port
    }

    func stop() {
        if process.isRunning { process.terminate() }
        process.waitUntilExit()
    }
}
