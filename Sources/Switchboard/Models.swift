import Foundation

enum Log {
    static var isEnabled: Bool { ProcessInfo.processInfo.environment["SWITCHBOARD_DEBUG"] == "1" }

    static func debug(_ text: String) {
        guard isEnabled else { return }
        FileHandle.standardError.write("[SB] \(text)\n".data(using: .utf8)!)
    }
}

func emailsMatch(_ lhs: String, _ rhs: String) -> Bool {
    lhs.caseInsensitiveCompare(rhs) == .orderedSame
}

/// 账号认证状态
enum AuthState: String, Codable {
    case ok
    case expired
    case unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = AuthState(rawValue: raw) ?? .unknown
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }

    var label: String {
        switch self {
        case .ok: return "生效中"
        case .expired: return "已失效"
        case .unknown: return "待确认"
        }
    }
}

/// 单个账号记录（需求文档 11 章约定的字段）
struct AccountRecord: Codable, Identifiable, Equatable {
    var id: UUID = UUID()
    var email: String
    var plan: String?
    var isCurrent: Bool = false
    var usedDays: Int?
    var totalDays: Int?
    var lastQuotaUpdate: Date?
    var lastError: String?
    var authState: AuthState = .unknown
    /// 仅在 token 未证实有效时提供重新导入。
    var showsReimport: Bool { authState != .ok }

    /// 头像字母：取邮箱 @ 前第一段的前两个字符大写
    var avatarLetters: String {
        let local = email.split(separator: "@").first.map(String.init) ?? email
        let letters = local.prefix(2)
        return String(letters).uppercased()
    }
}

/// 开机自启以系统实际状态为准，不写入 store.json。
struct AppSettings {
    var launchAtLogin = false
}

/// 持久化文件的整体结构。旧文件里的 settings 字段会被忽略。
struct StoreFile: Codable {
    static let currentVersion = 1
    var storeVersion: Int = 1
    var accounts: [AccountRecord] = []
}
