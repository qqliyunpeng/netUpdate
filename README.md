# netUpdate · AQ300 网络升级工具

这是 `D:\work_nvme\python_learn\pyside6\02.auto_tools` 的 Tauri 重写版。界面与
`D:\work_nvme\tauri`（noteCalc）保持同一套窗口和交互骨架：

- 无边框窗口 + 自绘标题栏，默认窗口为紧凑的 370 × 380（最小 360 × 370），
  沿用 noteCalc 的 `decorations: false`、居中启动、可缩放、启动时隐藏后再显示。
- 标题栏跟随窗口焦点、Windows 系统强调色和系统标题栏高度；菜单栏、页签高度由 Rust
  侧读取系统度量后注入 CSS。
- 全局禁用 WebView 默认右键菜单；输入框使用自绘的“撤销 / 重做 / 剪切 / 复制 / 粘贴 /
  全选”菜单，日志区使用“复制 / 全选 / 清空显示”菜单，菜单会自动避让窗口边缘。
- 配置、窗口位置和窗口尺寸会持久化；关闭窗口时再次落盘。

## 功能

- 本地文件升级：PAK 路径可以指向完整文件，也可以指向目录，目录会自动拼接
  `UPDATE_AQ300.PAK`。
- FTP 拉取：通过 `suppaftp` 从 FTP 后台下载 `UPDATE_AQ300.PAK`，不占用界面线程，
  文件保存到应用数据目录的 `tmp/` 下。
- TCP 升级：向 `<设备 IP>:9000` 分块发送 PAK，协议与 Python 版一致：
  - 32 字节小端头，分包 4096 字节；
  - 连接超时 12 秒、最多 3 次；设备响应等待 120 秒；
  - 响应 36 字节，最后一个 `u32` 为状态码，`0` 表示成功。
- 实时日志、进度条、Alt+U 快捷升级、FTP 设置弹窗、浅色 / 深色主题、关于和打开程序目录。
- 保留原工具的“网络升级”和“鼠标操作”两个页签；原版“鼠标操作”页没有业务控件，
  这里保留为说明页。

## 运行

```bash
pnpm install
pnpm dev                 # 浏览器预览，只调界面；不能执行网络升级
pnpm tauri dev           # 开发版窗口
pnpm tauri build --no-bundle
pnpm start               # 构建并启动 release 版
```

也可以双击 `dev.bat`、`preview.bat` 或 `run.bat`。

## 数据与配置

配置文件由 Rust 写入：

```text
%APPDATA%\com.localtools.netupdate\session.json
```

FTP 下载文件：

```text
%APPDATA%\com.localtools.netupdate\tmp\UPDATE_AQ300.PAK
```

首次启动时如果没有 `session.json`，程序会在可执行文件目录和当前工作目录查找旧版
`config.ini` 并导入以下字段：

```ini
[DEFAULT]
ip = 172.16.10.110
file_path = Z:/UPDATE_AQ300.PAK
ftp_host = 172.16.9.6
ftp_usrName = lyp
ftp_passwd = 123456
ftp_folder = /
use_local = 1
theme = dark
```

`use_local = 1` 使用本地文件，`0` 使用 FTP 拉取的文件。FTP 密码和原版一样以明文保存在
配置中，建议只在受控环境使用。

## 代码结构

```text
src/
  App.tsx                    会话、快捷键、菜单、升级流程
  components/
    TitleBar.tsx             自绘标题栏（焦点/系统强调色/最大化）
    MenuBar.tsx              应用菜单
    EditContextMenu.tsx      输入框与只读日志共用的右键菜单
    EditInput.tsx            带右键菜单的输入框
    UpgradePage.tsx          升级配置与日志布局
    LogView.tsx              只读日志、进度条、清空动作
    TabBar.tsx               两个功能页签
    StatusBar.tsx            状态栏
  lib/ipc.ts                 Tauri invoke / 事件 / 窗口 API 与预览降级
  lib/types.ts               前后端共享的数据形状
  styles/                    主题令牌与界面样式
src-tauri/src/
  lib.rs                     Tauri 命令、窗口几何恢复
  config.rs                  session.json、旧 config.ini 导入、路径解析
  upgrade.rs                 TCP 升级协议
  ftp.rs                     FTP 下载
  metrics.rs                 系统窗口度量
  theme.rs                   系统标题栏配色
  shortcuts.rs               WebView2 快捷键处理
```

## 说明

- 前端不直接读文件或发网络请求；文件对话框、FTP、TCP 和配置读写都在 Rust 侧完成。
- 浏览器预览模式会显示“浏览器预览 · 升级操作不可用”，升级和 FTP 按钮禁用。
- 与原 Python 版相比，窗口改为 noteCalc 风格的无边框自绘窗口，配置由 `config.ini`
  改为应用数据目录下的 `session.json`（仍支持旧文件导入），并增加了 FTP 设置面板、
  传输进度和日志右键菜单。
