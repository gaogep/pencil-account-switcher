import AppKit
import Foundation

/// 目标软件登录态快照。当前只含会话文件；
/// Pen 的供应商凭证（agent-auth）与 Pen 账号身份无关，刻意不入快照。
struct TargetSnapshot: Codable, Sendable {
    /// session-desktop.json 的原始内容
    var sessionFile: String
    /// 从会话文件解析出的邮箱（身份标识）
    var email: String
    /// 会话 token（额度接口的 Bearer 凭证）；旧版快照可能缺失
    var token: String?
}

enum TargetAdapterError: LocalizedError {
    case notLoggedIn
    case snapshotNotFound
    case verifyFailed(expected: String, actual: String?)
    case targetDidNotQuit
    case rollbackFailed
    case applicationMissing
    case relaunchFailed(original: String, launch: String)

    var errorDescription: String? {
        switch self {
        case .notLoggedIn: return "目标软件当前没有登录账号"
        case .snapshotNotFound: return "该账号的登录凭据缺失，请重新添加"
        case .verifyFailed(let e, let a): return "切换后身份校验失败（期望 \(e)，实际 \(a ?? "无")）"
        case .targetDidNotQuit: return "目标软件未能退出，可能正在等待保存确认。请先退出 Pen 后再切换"
        case .rollbackFailed: return "切换失败，原登录状态恢复失败，请在 Pen 中重新登录"
        case .applicationMissing: return "找不到 Pen，请先安装后再切换"
        case .relaunchFailed(let original, let launch):
            return "\(original)（恢复启动也失败：\(launch)）"
        }
    }
}

/// 目标软件适配层。主程序只依赖此协议，不出现任何具体软件的细节
///（需求文档把目标软件保持抽象，15 章要求接入方式可替换）。
protocol TargetAdapter {
    func liveIdentity() -> String?
    func captureSnapshot() throws -> TargetSnapshot
    func applySnapshot(_ snapshot: TargetSnapshot) throws
    func isRunning() -> Bool
    func terminateAndWait(timeout: TimeInterval) async -> Bool
    func canLaunch() -> Bool
    func launch() async throws
}

/// Pen 桌面版适配器（基于 2026-10-30 逆向结论）：
/// - 登录态 = ~/.pencil/session-desktop.json（{email, token}，纯文件读写）
/// - 运行中不可写文件（内存态会回写），必须先退出再落盘
/// - deviceId 由机器派生且不落盘，换会话文件不影响设备标识
final class PenTargetAdapter: TargetAdapter {
    static let bundleID = "dev.pencil.desktop"

    private let sessionFileURL: URL
    private let fallbackAppURL = URL(fileURLWithPath: "/Applications/Pen.app")

    init(home: URL = FileManager.default.homeDirectoryForCurrentUser) {
        sessionFileURL = home
            .appendingPathComponent(".pencil", isDirectory: true)
            .appendingPathComponent("session-desktop.json")
    }

    private struct SessionFile: Decodable {
        let email: String?
        let token: String?
    }

    private func sessionData() throws -> (data: Data, session: SessionFile) {
        let data = try Data(contentsOf: sessionFileURL)
        return (data, try JSONDecoder().decode(SessionFile.self, from: data))
    }

    func liveIdentity() -> String? {
        guard let result = try? sessionData(), let email = result.session.email,
              result.session.token?.isEmpty == false else { return nil }
        return email
    }

    func captureSnapshot() throws -> TargetSnapshot {
        guard let result = try? sessionData(), let email = result.session.email,
              let token = result.session.token, !token.isEmpty else {
            throw TargetAdapterError.notLoggedIn
        }
        return TargetSnapshot(sessionFile: String(decoding: result.data, as: UTF8.self), email: email, token: token)
    }

    func applySnapshot(_ snapshot: TargetSnapshot) throws {
        let data = Data(snapshot.sessionFile.utf8)
        let session = try JSONDecoder().decode(SessionFile.self, from: data)
        guard let email = session.email, emailsMatch(email, snapshot.email), session.token?.isEmpty == false else {
            throw TargetAdapterError.verifyFailed(expected: snapshot.email, actual: session.email)
        }
        // 原子写：临时文件 + rename，失败不会留下半个会话文件
        let directory = sessionFileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try data.write(to: sessionFileURL, options: .atomic)

        // 落盘后校验身份与快照一致
        guard let live = liveIdentity(), emailsMatch(live, snapshot.email) else {
            throw TargetAdapterError.verifyFailed(expected: snapshot.email, actual: liveIdentity())
        }
    }

    func isRunning() -> Bool {
        NSRunningApplication.runningApplications(withBundleIdentifier: Self.bundleID).contains { !$0.isTerminated }
    }

    func terminateAndWait(timeout: TimeInterval) async -> Bool {
        let apps = NSRunningApplication.runningApplications(withBundleIdentifier: Self.bundleID)
        guard !apps.isEmpty else { return true }
        apps.forEach { $0.terminate() }
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if Task.isCancelled { return false }
            if !isRunning() { return true }
            do {
                try await Task.sleep(nanoseconds: 200_000_000)
            } catch {
                return false
            }
        }
        return !isRunning()
    }

    func canLaunch() -> Bool { resolvedAppURL() != nil }

    func launch() async throws {
        guard let url = resolvedAppURL() else { throw TargetAdapterError.applicationMissing }
        _ = try await NSWorkspace.shared.openApplication(
            at: url, configuration: NSWorkspace.OpenConfiguration())
    }

    /// 按 bundle id 找已安装的 Pen。找不到再看 /Applications/Pen.app。
    private func resolvedAppURL() -> URL? {
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: Self.bundleID) {
            return url
        }
        if FileManager.default.fileExists(atPath: fallbackAppURL.path) { return fallbackAppURL }
        return nil
    }
}
