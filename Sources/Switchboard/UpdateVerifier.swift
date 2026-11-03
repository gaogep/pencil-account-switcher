import CryptoKit
import Foundation

enum UpdateVerifier {
    static let expectedBundleID = "dev.switchboard.Switchboard"

    /// `override` 供测试注入公钥。正式运行时从 Info.plist 的 SBUpdatePublicKey 读取。
    static func publicKey(from override: Data? = nil) throws -> Curve25519.Signing.PublicKey {
        let raw: Data
        if let override {
            raw = override
        } else if let b64 = Bundle.main.object(forInfoDictionaryKey: "SBUpdatePublicKey") as? String,
                  let decoded = Data(base64Encoded: b64) {
            raw = decoded
        } else {
            throw updateError("缺少更新公钥，无法校验更新包")
        }
        do {
            return try Curve25519.Signing.PublicKey(rawRepresentation: raw)
        } catch {
            throw updateError("缺少更新公钥，无法校验更新包")
        }
    }

    static func verifySignature(zip: URL, signatureBase64: String,
                                key: Curve25519.Signing.PublicKey) throws {
        guard let sig = Data(base64Encoded: signatureBase64.trimmingCharacters(in: .whitespacesAndNewlines)),
              key.isValidSignature(sig, for: try Data(contentsOf: zip)) else {
            throw updateError("更新包签名校验失败，已中止安装")
        }
    }

    static func verifyBundle(_ app: URL, expectedVersion: String) throws {
        guard let bundle = Bundle(url: app),
              bundle.bundleIdentifier == expectedBundleID,
              let version = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String,
              UpdateVersion.compare(version, expectedVersion) == .orderedSame else {
            throw updateError("更新包内容与发布版本不符")
        }
    }

    static func updateError(_ text: String) -> NSError {
        NSError(domain: "SwitchboardUpdate", code: 10, userInfo: [NSLocalizedDescriptionKey: text])
    }
}
