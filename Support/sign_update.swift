import CryptoKit
import Foundation

// 用法：UPDATE_SIGNING_KEY=<base64> swift Support/sign_update.swift <zip路径>
// 私钥是 32 字节 Ed25519 seed 的 base64，放在 GitHub Secret UPDATE_SIGNING_KEY，不要写入仓库。
guard CommandLine.arguments.count == 2,
      let keyB64 = ProcessInfo.processInfo.environment["UPDATE_SIGNING_KEY"],
      let keyData = Data(base64Encoded: keyB64) else {
    FileHandle.standardError.write("用法: UPDATE_SIGNING_KEY=... swift sign_update.swift <zip>\n".data(using: .utf8)!)
    exit(1)
}

do {
    let zipURL = URL(fileURLWithPath: CommandLine.arguments[1])
    let key = try Curve25519.Signing.PrivateKey(rawRepresentation: keyData)
    let signature = try key.signature(for: Data(contentsOf: zipURL))
    let sigURL = zipURL.appendingPathExtension("sig")
    try signature.base64EncodedString().write(to: sigURL, atomically: true, encoding: .utf8)
    print(sigURL.path)
} catch {
    FileHandle.standardError.write("签名失败：\(error)\n".data(using: .utf8)!)
    exit(1)
}
