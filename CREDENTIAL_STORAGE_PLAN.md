# 凭据存储改造方案（Keychain → 本地受保护文件存储）

## 1. 背景与目标

### 现状问题
当前项目使用 `codesign --force --sign -`（ad-hoc 临时签名）构建，无 Apple Developer 团队证书与 `keychain-access-groups` entitlement，导致无法使用新式 Data Protection Keychain，只能回退至 macOS 传统登录钥匙串（Login Keychain）。
传统钥匙串依赖二进制 `cdhash` 校验访问权限，每次重新编译或升级应用后 `cdhash` 都会改变，导致在**刷新额度**、**切换账号**时系统频繁弹出“Pencil Account Switcher 想要使用你储存在钥匙串中的机密信息”的密码授权框。

### 安全边界评估
目标软件 Pen 桌面版自身的活跃登录态原本即以明文 JSON 存储在 `~/.pencil/session-desktop.json`（含 `email` 与 `token`）。因此将切号器的账号快照改为存储在当前用户专属的应用支持目录下，安全水位与 Pen 官方客户端完全一致，且能彻底消除钥匙串弹窗，更适合开源分发与本地自编译。

### 改造目标
1. **彻底移除钥匙串依赖**：删除 `KeychainStore.swift` 中的 `Security` / `LocalAuthentication` 调用，不再读写 macOS Keychain。
2. **改用本地受保护文件存储（方案 A）**：保持 [`CredentialStore`](file:///Users/apple/Documents/pen_switch_account/Sources/Switchboard/KeychainStore.swift#L21-L25) 协议不变，将每个账号的快照独立保存在 `~/Library/Application Support/Switchboard/credentials/<account-uuid>.json`。
3. **做好文件权限与隔离防护**：
   - 凭据与 `store.json` 分离存放（避免用户分享配置排障时泄露 token）。
   - `credentials` 目录权限强制设为 `0700`（仅当前系统用户可进入/读写），并标记 `isExcludedFromBackup = true`（不进入 Time Machine 等备份）。
   - 单个凭据文件权限强制设为 `0600`（仅当前系统用户可读写），采用 `.atomic` 原子写入。
   - 严格校验 `account` 参数必须为合法 `UUID`，防止路径穿越。

---

## 2. 涉及文件清单

| 文件路径 | 操作 | 说明 |
|---|---|---|
| [`Sources/Switchboard/KeychainStore.swift`](file:///Users/apple/Documents/pen_switch_account/Sources/Switchboard/KeychainStore.swift) | 重写（或重命名为 `CredentialStore.swift`） | 移除 `KeychainStore` 与 `KeychainError`，实现 `FileCredentialStore` 与 `CredentialError` |
| [`Sources/Switchboard/AppModel.swift`](file:///Users/apple/Documents/pen_switch_account/Sources/Switchboard/AppModel.swift) | 修改 | 默认注入改为 `FileCredentialStore()`，更新 `loadSnapshot` 的错误捕获类型 |
| [`Tests/RegressionTests.swift`](file:///Users/apple/Documents/pen_switch_account/Tests/RegressionTests.swift) | 修改 | `MemoryCredentials` 错误类型改为 `CredentialError.notFound`，新增 `FileCredentialStore` 单元测试 |
| [`Tests/run.sh`](file:///Users/apple/Documents/pen_switch_account/Tests/run.sh) | 按需修改 | 若重命名了 `KeychainStore.swift`，同步更新编译文件列表 |
| [`README.md`](file:///Users/apple/Documents/pen_switch_account/README.md) | 修改 | 更新架构说明中的凭据存储描述 |

---

## 3. 详细改造步骤

### 步骤一：替换凭据存储实现（`Sources/Switchboard/KeychainStore.swift`）

保留 `CredentialStore` 协议签名不变，用基于本地文件系统的 `FileCredentialStore` 替换原有的 `KeychainStore`（可原地替换文件内容，或将文件重命名为 `CredentialStore.swift`）：

```swift
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
```

---

### 步骤二：更新 `Sources/Switchboard/AppModel.swift`

1. **修改默认参数（约第 61–67 行）**：
   将 `init` 中的默认 `keychain: CredentialStore = KeychainStore()` 改为 `FileCredentialStore()`：
   ```swift
   init(adapter: TargetAdapter = PenTargetAdapter(), quota: QuotaProvider = PenQuotaProvider(),
        keychain: CredentialStore = FileCredentialStore(), storeURL: URL? = nil, webLogin: WebLoginProvider? = nil,
        softVerifyNanoseconds: UInt64 = 4_000_000_000)
   ```
   > 注：参数名 `keychain` 可保留以减少测试调用方的改动面，或统一重命名为 `credentials`。

2. **修改 `loadSnapshot(for:)` 错误处理（约第 466–482 行）**：
   将捕获 `KeychainError` 改为捕获 `CredentialError`：
   ```swift
   private func loadSnapshot(for id: UUID) throws -> TargetSnapshot {
       let data: Data
       do {
           data = try keychain.load(account: id.uuidString)
       } catch let error as CredentialError {
           switch error {
           case .notFound, .invalidAccount:
               throw TargetAdapterError.snapshotNotFound
           }
       }
       do {
           return try JSONDecoder().decode(TargetSnapshot.self, from: data)
       } catch {
           throw TargetAdapterError.snapshotNotFound
       }
   }
   ```

---

### 步骤三：更新测试与构建脚本

1. **[`Tests/RegressionTests.swift`](file:///Users/apple/Documents/pen_switch_account/Tests/RegressionTests.swift)**：
   - 第 8 行：`MemoryCredentials.load` 中将 `throw KeychainError.notFound` 改为 `throw CredentialError.notFound`。
   - 在 `run()` 末尾补充针对 `FileCredentialStore` 的回归测试：
     - 验证 `save` → `load` 能完整读回数据。
     - 验证生成的目录权限为 `0o700`、文件权限为 `0o600`。
     - 验证传入非 UUID 字符串（如 `"../evil"`）会抛出 `CredentialError.invalidAccount`。
     - 验证 `delete` 后再次 `load` 抛出 `CredentialError.notFound`，且重复 `delete` 幂等不报错。

2. **[`Tests/run.sh`](file:///Users/apple/Documents/pen_switch_account/Tests/run.sh)**：
   - 若将 `KeychainStore.swift` 重命名为了 `CredentialStore.swift`，需同步修改第 7 行的编译文件列表。

3. **[`README.md`](file:///Users/apple/Documents/pen_switch_account/README.md)**：
   - 更新目录结构与“凭据”章节说明，将 macOS Keychain 改为 `~/Library/Application Support/Switchboard/credentials/<uuid>.json`（`0700`/`0600` 权限保护）。

---

## 4. 验收标准

1. 运行 `./Tests/run.sh`，全部回归测试（含新增的 `FileCredentialStore` 权限与读写测试）100% 通过。
2. 运行 `./build.sh` 编译打包成功，无任何编译警告（原先 `SecAccessCreate` 的废弃警告不再存在）。
3. 全库检索 `import Security`、`import LocalAuthentication`、`SecItem` 均无残留引用。
4. 实际运行应用执行添加账号、手动刷新额度、切换账号、移除账号，全程不再弹出 macOS 钥匙串授权弹窗。
