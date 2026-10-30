import CryptoKit
import Darwin
import Foundation

enum QuotaResult {
    case quota(used: Int, total: Int, plan: String?)
    /// 额度不计量/数据缺失：界面显示「— / — 天」，不伪造数据
    case unsupported
    case expired
    case failure(String)
}

/// 额度读取抽象（需求 05 章：以目标软件服务端数据为准，不从本地推算）
protocol QuotaProvider {
    func fetchQuota(snapshot: TargetSnapshot) async -> QuotaResult
}

/// 占位实现（演示模式/接口不可用时）
struct UnsupportedQuotaProvider: QuotaProvider {
    func fetchQuota(snapshot: TargetSnapshot) async -> QuotaResult { .unsupported }
}

/// Pen 官方额度接口（逆向自 Pen.app 1.2.21，2026-10-30 实测）：
/// POST https://api.pencil.dev/public/usage-status
/// body {featureKey:"mcp_usage_days", timeZone}
/// headers Authorization: Bearer <sessionToken> / X-Device-Id
/// → {allowed, isMetered, billingEnabled, used, limit, remaining}
/// usage-status 为只查询通道（usage-consume 才会计量扣减，切号器不使用）。
final class PenQuotaProvider: QuotaProvider {
    struct Response: Decodable {
        let isMetered: Bool?
        let used: Int?
        let limit: Int?
    }

    private let baseURL: URL

    init(baseURL: URL = URL(string: "https://api.pencil.dev")!) {
        self.baseURL = baseURL
    }

    func fetchQuota(snapshot: TargetSnapshot) async -> QuotaResult {
        guard let token = snapshot.token else {
            return .expired
        }
        var request = URLRequest(url: baseURL.appendingPathComponent("public/usage-status"))
        request.httpMethod = "POST"
        request.timeoutInterval = 20
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue(Self.deviceId(), forHTTPHeaderField: "X-Device-Id")
        let body: [String: String] = [
            "featureKey": "mcp_usage_days",
            "timeZone": TimeZone.current.identifier,
        ]
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            if let http = response as? HTTPURLResponse, let mapped = Self.result(forHTTPStatus: http.statusCode) {
                return mapped
            }
            return Self.parseBody(data)
        } catch {
            return .failure("网络错误：\(error.localizedDescription)")
        }
    }

    static func parseBody(_ data: Data) -> QuotaResult {
        guard let decoded = try? JSONDecoder().decode(Response.self, from: data) else {
            return .failure("额度数据解析失败")
        }
        guard decoded.isMetered != false else { return .unsupported }
        guard let used = decoded.used, let limit = decoded.limit else { return .unsupported }
        return .quota(used: used, total: limit, plan: nil)
    }

    /// 401 才是会话失效。403 多半是套餐没有这项额度，不能把账号标成过期。
    static func result(forHTTPStatus status: Int) -> QuotaResult? {
        switch status {
        case 401: return .expired
        case 403: return .failure("没有权限读取额度，登录状态保持不变")
        case 200..<300: return nil
        default: return .failure("额度接口返回 \(status)")
        }
    }

    /// 与 Pen 桌面端 getDesktopDeviceId 一致：md5(gethostname() + "darwin" + arch)。结果缓存，避免主线程做主机名解析。
    static func deviceId() -> String { cachedDeviceId }

    private static let cachedDeviceId: String = {
        #if arch(arm64)
        let arch = "arm64"
        #else
        let arch = "x64"
        #endif
        var buffer = [CChar](repeating: 0, count: 256)
        let host = gethostname(&buffer, buffer.count) == 0
            ? String(cString: buffer)
            : ProcessInfo.processInfo.hostName
        let digest = Insecure.MD5.hash(data: Data((host + "darwin" + arch).utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }()
}
