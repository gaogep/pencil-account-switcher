import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

struct AppRelease: Equatable {
    var version: String
    var zipURL: URL
    var pageURL: URL
    /// 与 zip 同名的 Ed25519 签名（base64）。缺省为 nil，方便旧测试构造。
    var signatureURL: URL? = nil
}

enum AppUpdateState: Equatable {
    case idle
    case available(AppRelease, silenced: Bool)
    case downloading(AppRelease, percent: Int, received: Int64, total: Int64?)
    case ready(AppRelease, unpackedApp: URL)

    var release: AppRelease? {
        switch self {
        case .idle: return nil
        case .available(let release, _), .downloading(let release, _, _, _), .ready(let release, _):
            return release
        }
    }

    var isBusy: Bool {
        switch self {
        case .downloading, .ready: return true
        default: return false
        }
    }

    var signature: String {
        switch self {
        case .idle: return "idle"
        case .available(let release, let silenced): return "available:\(release.version):\(silenced)"
        case .downloading(let release, _, _, _): return "downloading:\(release.version)"
        case .ready(let release, _): return "ready:\(release.version)"
        }
    }
}

enum UpdateVersion {
    static func normalized(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.first == "v" || trimmed.first == "V" { return String(trimmed.dropFirst()) }
        return trimmed
    }

    static func compare(_ lhs: String, _ rhs: String) -> ComparisonResult {
        let left = parts(normalized(lhs))
        let right = parts(normalized(rhs))
        let count = max(left.count, right.count)
        for index in 0..<count {
            let a = index < left.count ? left[index] : 0
            let b = index < right.count ? right[index] : 0
            if a < b { return .orderedAscending }
            if a > b { return .orderedDescending }
        }
        return .orderedSame
    }

    /// 只接受文件名里带 arm64、并且以 .zip 结尾的包。不再退回任意 zip。
    static func zipAsset(from names: [String]) -> String? {
        names.first { $0.lowercased().contains("arm64") && $0.lowercased().hasSuffix(".zip") }
    }

    static func formatBytes(_ value: Int64) -> String {
        if value < 1024 { return "\(value) B" }
        let kb = Double(value) / 1024
        if kb < 1024 { return String(format: "%.1f KB", kb) }
        return String(format: "%.1f MB", kb / 1024)
    }

    private static func parts(_ version: String) -> [Int] {
        version.split(separator: ".").map { Int($0) ?? 0 }
    }
}

protocol ReleaseFeed {
    func fetchLatest() async throws -> AppRelease
}

protocol UpdateInstaller {
    func unpackZip(_ zipURL: URL) throws -> URL
    func replace(current: URL, with newApp: URL) throws
    func relaunch(at url: URL)
}

enum UpdateInstallError: LocalizedError, Equatable {
    /// App Translocation 只读，或安装目录无写权限（例如直接在 DMG / 下载里跑）
    case locationNotWritable

    var errorDescription: String? {
        switch self {
        case .locationNotWritable:
            return "请先把应用拖到「应用程序」文件夹后再更新"
        }
    }
}

protocol UpdateTransport {
    func downloadZip(_ url: URL, onProgress: @escaping (Int64, Int64) -> Void) async throws -> URL
    func downloadSignature(_ url: URL) async throws -> String
}

struct SessionUpdateTransport: UpdateTransport {
    func downloadZip(_ url: URL, onProgress: @escaping (Int64, Int64) -> Void) async throws -> URL {
        let task = DownloadTask()
        task.onProgress = onProgress
        return try await task.download(url)
    }

    func downloadSignature(_ url: URL) async throws -> String {
        let (data, response) = try await URLSession.shared.data(from: url)
        if DownloadTask.errorIfUnsuccessful(response) != nil {
            throw NSError(domain: "SwitchboardUpdate", code: 11,
                          userInfo: [NSLocalizedDescriptionKey: "下载签名文件失败"])
        }
        return String(decoding: data, as: UTF8.self)
    }
}

struct GitHubReleaseFeed: ReleaseFeed {
    let repo: String
    let session: URLSession
    let apiBase: URL

    init(repo: String, session: URLSession = .shared,
         apiBase: URL = URL(string: "https://api.github.com")!) {
        self.repo = repo
        self.session = session
        self.apiBase = apiBase
    }

    func fetchLatest() async throws -> AppRelease {
        guard let url = URL(string: "\(apiBase.absoluteString)/repos/\(repo)/releases/latest") else {
            throw URLError(.badURL)
        }
        var request = URLRequest(url: url)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 20
        let (data, response) = try await session.data(for: request)
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw URLError(.badServerResponse)
        }
        let decoded = try JSONDecoder().decode(Payload.self, from: data)
        if decoded.prerelease { throw URLError(.cannotParseResponse) }
        let version = UpdateVersion.normalized(decoded.tagName)
        let names = decoded.assets.map(\.name)
        guard let zipName = UpdateVersion.zipAsset(from: names),
              let asset = decoded.assets.first(where: { $0.name == zipName }),
              let zipURL = URL(string: asset.browserDownloadURL),
              let sigAsset = decoded.assets.first(where: { $0.name == zipName + ".sig" }),
              let sigURL = URL(string: sigAsset.browserDownloadURL),
              let pageURL = URL(string: decoded.htmlURL) else {
            throw URLError(.cannotParseResponse)
        }
        return AppRelease(version: version, zipURL: zipURL, pageURL: pageURL, signatureURL: sigURL)
    }

    private struct Payload: Decodable {
        let tagName: String
        let htmlURL: String
        let prerelease: Bool
        let assets: [Asset]

        enum CodingKeys: String, CodingKey {
            case tagName = "tag_name"
            case htmlURL = "html_url"
            case prerelease, assets
        }

        struct Asset: Decodable {
            let name: String
            let browserDownloadURL: String
            enum CodingKeys: String, CodingKey {
                case name
                case browserDownloadURL = "browser_download_url"
            }
        }
    }
}

struct FileUpdateInstaller: UpdateInstaller {
    func unpackZip(_ zipURL: URL) throws -> URL {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("SwitchboardUpdate-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        process.arguments = ["-xk", zipURL.path, folder.path]
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw NSError(domain: "SwitchboardUpdate", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "解压更新包失败"])
        }
        return try Self.uniqueApp(in: folder)
    }

    /// 解压目录里必须恰好有一个 .app，避免装进包内附带的其他应用。
    static func uniqueApp(in folder: URL) throws -> URL {
        let apps = try FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "app" }
        guard apps.count == 1, let app = apps.first else {
            throw NSError(domain: "SwitchboardUpdate", code: 2,
                          userInfo: [NSLocalizedDescriptionKey: "更新包结构异常"])
        }
        return app
    }

    /// DMG / 下载里直接打开会进 App Translocation（只读）；无写权限的目录也无法替换。
    static func assertReplaceable(current: URL, fileManager: FileManager = .default) throws {
        let path = current.resolvingSymlinksInPath().path
        if path.contains("/AppTranslocation/") {
            throw UpdateInstallError.locationNotWritable
        }
        let parent = current.deletingLastPathComponent()
        guard fileManager.isWritableFile(atPath: parent.path) else {
            throw UpdateInstallError.locationNotWritable
        }
    }

    func replace(current: URL, with newApp: URL) throws {
        try Self.assertReplaceable(current: current)
        // 隔离标记在签名和 bundle 校验通过之后才清。调用方必须先验签。
        let cleaner = Process()
        cleaner.executableURL = URL(fileURLWithPath: "/usr/bin/xattr")
        cleaner.arguments = ["-cr", newApp.path]
        try? cleaner.run()
        cleaner.waitUntilExit()
        _ = try FileManager.default.replaceItemAt(current, withItemAt: newApp)
    }

    /// 等当前进程退出后再 open。旧进程还活着时，LaunchServices 只会把已有实例拉到前台。
    func relaunch(at url: URL) {
        let pid = ProcessInfo.processInfo.processIdentifier
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = [
            "-c",
            "while /bin/kill -0 \(pid) 2>/dev/null; do /bin/sleep 0.2; done; /usr/bin/open \"$1\"",
            "sh", url.path,
        ]
        try? process.run()
    }
}

final class DownloadTask: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<URL, Error>?
    private var task: URLSessionDownloadTask?
    var onProgress: ((Int64, Int64) -> Void)?

    func download(_ url: URL) async throws -> URL {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                let session = URLSession(configuration: .default, delegate: self, delegateQueue: nil)
                let task = session.downloadTask(with: url)
                lock.withLock {
                    self.continuation = continuation
                    self.task = task
                }
                if Task.isCancelled {
                    task.cancel()
                    return
                }
                task.resume()
            }
        } onCancel: {
            let task = lock.withLock { self.task }
            task?.cancel()
        }
    }

    /// 非 2xx（或根本没有 HTTP 响应）时返回带状态码的错误。
    static func errorIfUnsuccessful(_ response: URLResponse?) -> Error? {
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            let code = (response as? HTTPURLResponse)?.statusCode ?? -1
            return NSError(domain: "SwitchboardUpdate", code: code,
                           userInfo: [NSLocalizedDescriptionKey: "下载更新失败（HTTP \(code)）"])
        }
        return nil
    }

    private func finish(_ result: Result<URL, Error>) {
        let pending = lock.withLock { () -> CheckedContinuation<URL, Error>? in
            defer { continuation = nil }
            return continuation
        }
        pending?.resume(with: result)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didWriteData bytesWritten: Int64, totalBytesWritten: Int64,
                    totalBytesExpectedToWrite: Int64) {
        onProgress?(totalBytesWritten, totalBytesExpectedToWrite)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didFinishDownloadingTo location: URL) {
        if let error = Self.errorIfUnsuccessful(downloadTask.response) {
            finish(.failure(error))
            return
        }
        do {
            let copy = FileManager.default.temporaryDirectory
                .appendingPathComponent("Switchboard-\(UUID().uuidString).zip")
            try FileManager.default.copyItem(at: location, to: copy)
            finish(.success(copy))
        } catch {
            finish(.failure(error))
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error {
            let mapped: Error = (error as? URLError)?.code == .cancelled ? CancellationError() : error
            finish(.failure(mapped))
        }
        session.finishTasksAndInvalidate()
    }
}
