# AGENTS.md

本文件面向在此仓库中工作的代码代理。面向用户的产品说明、安装和使用文档在
`README.md`；不要把这些内容重复写进 README。

## 项目概览

- netUpdate 是 AQ300 网络升级工具的 Tauri 2 重写版，前端使用 React 19 +
  TypeScript，后端使用 Rust。
- 桌面 GUI 和 `netupdate-cli` 共用 Rust 侧的配置、FTP 下载和 TCP 升级实现。
- 默认目标是 Windows，但发布工作流同时构建 Windows、Linux 和 macOS 产物。
- 不要修改与当前任务无关的文件或行为；这个仓库按小而聚焦的提交演进。

## 常用命令

在仓库根目录运行：

```powershell
pnpm install
pnpm dev                         # Vite 浏览器预览，不能连接设备
pnpm tauri dev                   # 开发版 Tauri 窗口
pnpm build                       # tsc --noEmit + vite build
pnpm tauri build --no-bundle     # 快速检查 Rust/Tauri 集成
pnpm tauri build                 # 完整安装包
pnpm build:cli                   # release 版 netupdate-cli.exe
pnpm start                       # 必要时构建并启动 release 版
pnpm portable                    # 生成便携包
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

仓库目前没有独立的前端测试套件。修改前端后至少运行 `pnpm build`，涉及 Tauri
命令、窗口或 Rust 逻辑时还要运行对应的 `cargo test`。

## 代码地图

### 前端 `src/`

- `App.tsx`：会话加载、快捷键、菜单、升级流程和日志状态。
- `lib/ipc.ts`：Tauri `invoke`、事件和窗口 API 的唯一前端入口，同时提供浏览器
  预览降级。
- `lib/types.ts`：前后端共享的数据形状和默认协议值。
- `lib/protocol.ts`：协议摘要和默认值判断。
- `components/UpgradePage.tsx`：升级表单、FTP 设置和协议入口。
- `components/LogView.tsx`：日志、进度条和日志右键菜单。
- `components/ProtocolModal.tsx`：传输参数与协议字段编辑。
- `styles/`：主题令牌和界面样式。

### Rust `src-tauri/src/`

- `lib.rs`：Tauri 命令注册、窗口几何恢复和前后端桥接。
- `upgrade.rs`：TCP 升级协议、连接重试、进度和日志事件。
- `ftp.rs`：FTP 下载。
- `config.rs`：`session.json`、旧版 `config.ini` 导入和路径解析。
- `cli.rs`：`netupdate-cli` 子命令、JSON 输出和 FTP 参数覆盖。
- `bin/netupdate-cli.rs`：CLI 入口。
- `metrics.rs`、`theme.rs`、`shortcuts.rs`：Windows 标题栏、系统度量和 WebView
  快捷键适配。

## 架构约束

- 前端不直接访问文件系统或网络。文件选择、配置读写、FTP、TCP 和设备状态都必须在
  Rust 侧完成，再通过 `src/lib/ipc.ts` 暴露给 React。
- GUI 和 CLI 必须复用 `config.rs`、`ftp.rs`、`upgrade.rs` 的实现。不要为 CLI
  复制一套协议或配置逻辑。
- 前后端数据通过 `serde(rename_all = "camelCase")` 对接；新增字段时同步更新
  `src/lib/types.ts`、Rust 结构体和序列化测试。
- 浏览器预览必须继续可用。新增 Tauri 调用时，在 `src/lib/ipc.ts` 中为非 Tauri
  环境提供合理的降级路径，不能让预览页直接报错。
- 不要提交 `node_modules/`、`dist/`、`src-tauri/target/` 或生成的
  `src-tauri/gen/schemas/`。
- `.cargo/config.toml` 与本机 MinGW 链接器绑定，已被忽略。Windows GNU 工具链
  出现 `corrupt .drectve` 或 `ld returned 5` 时，运行
  `scripts/setup-linker.ps1`，不要把它当作业务代码问题。

## 升级协议不变量

- 默认常量同时存在于 `src/lib/types.ts` 和 `src-tauri/src/upgrade.rs`；修改协议
  默认值、范围校验或字段时，两边和测试必须一起更新。
- 请求头为小端 32 字节，随后是 PAK 分块；默认分块 4096 字节、魔数 `0x9000`、
  包类型 `0x0002`、状态码偏移 32。
- 设备响应为 36 字节，状态码是响应中指定偏移处的 `u32`；`0` 表示成功。
- 一次升级只能建立一条业务 TCP 连接。这条连接既用于可达性判断，也用于发送数据。
  不要恢复“先探测、再重连”的两连接方案，否则单连接设备可能拒绝正式升级。
- 连接失败时按 `connectAttempts` 重试；前几次使用较短超时，最后一次使用完整超时。
  修改重试逻辑时要覆盖超时、设备重启后重连和日志可读性。
- 协议参数必须经过 `ProtocolSettings::validate` 校验，CLI 和 GUI 都不能绕过。

## CLI 契约

- 全局参数 `--json` 必须放在子命令之前，例如
  `netupdate-cli.exe --json upgrade ...`。
- 使用 `--json` 时，stdout 只能有一个完整的 JSON 对象；日志和进度写入 stderr。
- 成功返回 `0`，失败返回 `1`；失败 JSON 至少包含 `ok: false`、`operation` 和
  `error`。
- `config` 只能输出 `ftpPasswordSet`，不得输出明文 FTP 密码。
- FTP 密码优先从 `NETUPDATE_FTP_PASSWORD` 读取；不要在日志、示例或测试快照中写真实
  密码。
- 新增 CLI 输出字段时保持向后兼容，并同步更新 README 中的用法。

代理调用 CLI 时，优先使用 `--json` 并检查退出码；实际升级前先执行 `--dry-run`，
向真实设备写入前必须取得用户明确确认。

## 配置与迁移

- 配置位于应用数据目录的 `com.localtools.netupdate/session.json`，FTP 下载文件位于
  同目录的 `tmp/`。
- `session.json` 保存配置、协议参数和窗口几何；`LAYOUT_VERSION` 变更时必须考虑旧
  窗口尺寸迁移。
- 没有 `session.json` 时，仍要支持从可执行文件目录或当前目录导入旧版
  `config.ini`。
- FTP 密码按现有设计以明文保存。不要把它写进 README 截图、测试输出或日志。
- 测试和截图必须使用临时目录或演示数据，不得改写用户真实的 `session.json`。

## 界面约定

- 主窗口是紧凑布局：默认 370 × 380，最小 360 × 370，无系统边框，使用自绘标题栏。
- 标题栏、菜单栏和页签高度由 Rust 的系统度量注入，不要在 CSS 中再写一套互相冲突的
  固定值。
- 输入框和日志区使用自绘右键菜单；不要重新引入 WebView 默认上下文菜单。
- 主题同时支持浅色和深色，新增颜色优先使用 `styles/theme.css` 中的令牌。
- UI 文案使用中文，并保持现有简洁、工具化的语气。
- 主界面变化后，更新 `docs/images/netupdate.png`。截图使用演示 IP、路径和日志，
  不要暴露本机真实路径、设备地址或 FTP 密码。

## 验证与文档

提交前按改动范围执行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm build
pnpm tauri build --no-bundle   # 改动 Tauri 配置、命令或窗口行为时
```

README 保持用户视角：介绍产品、安装、使用、CLI、配置和构建即可。实现细节、架构
约束、测试策略和 AI 调用规则放在本文件。不要把真实凭据或个人路径写进任何文档或图片。
