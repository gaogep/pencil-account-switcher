import Foundation

enum CredentialError: LocalizedError, Equatable {
    case notFound
    case invalidAccount

    var errorDescription: String? {
        switch self {
        case .notFound: return "本地没有该账号的登录凭据"
        case .invalidAccount: return "凭据账号标识无效"
        }
    }
}

/// 账号登录凭据的存储：快照整体作为独立文件存入 Application Support/Switchboard/credentials，
/// 普通配置文件（store.json）只保留账号元数据与引用。
protocol CredentialStore {
    func save(_ data: Data, account: String) throws
    func load(account: String) throws -> Data
    func delete(account: String) throws
}

final class FileCredentialStore: CredentialStore {
    private let directoryURL: URL
    private let fileManager = FileManager.default

    init(directoryURL: URL? = nil) {
        self.directoryURL = directoryURL ?? FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Switchboard", isDirectory: true)
            .appendingPathComponent("credentials", isDirectory: true)
    }

    func save(_ data: Data, account: String) throws {
        let url = try fileURL(for: account)
        try ensureDirectory()
        try data.write(to: url, options: .atomic)
        try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }

    func load(account: String) throws -> Data {
        let url = try fileURL(for: account)
        guard fileManager.fileExists(atPath: url.path) else {
            throw CredentialError.notFound
        }
        return try Data(contentsOf: url)
    }

    func delete(account: String) throws {
        let url = try fileURL(for: account)
        guard fileManager.fileExists(atPath: url.path) else { return }
        try fileManager.removeItem(at: url)
    }

    /// 仅接受合法 UUID 字符串作为文件名，防止路径穿越
    private func fileURL(for account: String) throws -> URL {
        guard let uuid = UUID(uuidString: account) else {
            throw CredentialError.invalidAccount
        }
        return directoryURL.appendingPathComponent("\(uuid.uuidString).json", isDirectory: false)
    }

    private func ensureDirectory() throws {
        if !fileManager.fileExists(atPath: directoryURL.path) {
            try fileManager.createDirectory(
                at: directoryURL,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            var resourceValues = URLResourceValues()
            resourceValues.isExcludedFromBackup = true
            var mutableURL = directoryURL
            try? mutableURL.setResourceValues(resourceValues)
        } else {
            try fileManager.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directoryURL.path)
        }
    }
}
