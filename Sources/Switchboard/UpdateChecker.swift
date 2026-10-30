import AppKit
import Foundation

struct AppRelease: Equatable {
    var version: String
    var zipURL: URL
    var pageURL: URL
}

enum AppUpdateState: Equatable {
    case idle
    case available(AppRelease, silenced: Bool)
    case downloading(AppRelease, percent: Int, received: Int64, total: Int64?)
    case ready(AppRelease, unpackedApp: URL)
    case failed(String)

    var release: AppRelease? {
        switch self {
        case .idle, .failed: return nil
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
        case .failed: return "failed"
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

    static func zipAsset(from names: [String]) -> String? {
        names.first { $0.lowercased().contains("arm64") && $0.lowercased().hasSuffix(".zip") }
            ?? names.first { $0.lowercased().hasSuffix(".zip") }
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

struct GitHubReleaseFeed: ReleaseFeed {
    let repo: String
    let session: URLSession

    init(repo: String, session: URLSession = .shared) {
        self.repo = repo
        self.session = session
    }

    func fetchLatest() async throws -> AppRelease {
        guard let url = URL(string: "https://api.github.com/repos/\(repo)/releases/latest") else {
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
              let pageURL = URL(string: decoded.htmlURL) else {
            throw URLError(.cannotParseResponse)
        }
        return AppRelease(version: version, zipURL: zipURL, pageURL: pageURL)
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
        let apps = try FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "app" }
        guard let app = apps.first else {
            throw NSError(domain: "SwitchboardUpdate", code: 2,
                          userInfo: [NSLocalizedDescriptionKey: "更新包里没有应用"])
        }
        return app
    }

    func replace(current: URL, with newApp: URL) throws {
        let cleaner = Process()
        cleaner.executableURL = URL(fileURLWithPath: "/usr/bin/xattr")
        cleaner.arguments = ["-cr", newApp.path]
        try? cleaner.run()
        cleaner.waitUntilExit()
        _ = try FileManager.default.replaceItemAt(current, withItemAt: newApp)
    }

    func relaunch(at url: URL) {
        NSWorkspace.shared.open(url)
    }
}

final class DownloadTask: NSObject, URLSessionDownloadDelegate {
    private var continuation: CheckedContinuation<URL, Error>?
    private var destination: URL?
    var onProgress: ((Int64, Int64) -> Void)?

    func download(_ url: URL) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            self.continuation = continuation
            let session = URLSession(configuration: .default, delegate: self, delegateQueue: nil)
            session.downloadTask(with: url).resume()
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didWriteData bytesWritten: Int64, totalBytesWritten: Int64,
                    totalBytesExpectedToWrite: Int64) {
        onProgress?(totalBytesWritten, totalBytesExpectedToWrite)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didFinishDownloadingTo location: URL) {
        do {
            let copy = FileManager.default.temporaryDirectory
                .appendingPathComponent("Switchboard-\(UUID().uuidString).zip")
            try FileManager.default.copyItem(at: location, to: copy)
            continuation?.resume(returning: copy)
        } catch {
            continuation?.resume(throwing: error)
        }
        continuation = nil
        session.finishTasksAndInvalidate()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error {
            continuation?.resume(throwing: error)
            continuation = nil
        }
        session.finishTasksAndInvalidate()
    }
}
