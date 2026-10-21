# Pare 代理说明

本地桌面应用：压图，或一次换成 WebP / JPEG / PNG。标识 `app.pare.desktop`。界面跟随系统语言，提供简体中文和 English，可在设置里改。产品名是英文 Pare。

优先级：用户当前指令 > 本仓库正在用的代码 > `docs/`。文档和代码不一致时，以代码为准，不要把界面改回文档里的旧说法，除非用户明确要求。

## 不要做的事

- 不要改原图。结果只写到输出文件夹
- 不要上传图片，不要加账号、统计、自动更新
- 不要先有损再转码。换格式必须从原图编码一次
- 同格式只在结果更小时替换，否则复制原图并标「未变小」
- 不要覆盖已有输出。重名用 `-2`、`-3`
- 取消不能留下半截输出文件
- 不要修改、移动或重装 `/Applications/ImageOptim.app`，也不要重装 rustup
- 不要改 MWeb 笔记库和 `/Users/lzt/Desktop/knowledge`
- 未要求时不要提交 git，不要改 `docs/` 里的三份说明

第一版不做：访达右键、AVIF、JPEG XL、改尺寸、裁切、水印、并排预览、命令行、HEIC / TIFF / BMP / PDF。

## 技术栈

- 包管理：pnpm。Node 20 及以上
- 界面：Vue 3.5、`<script setup>`、TypeScript、Vite、Pinia。没有 Vue Router，也没有 Tailwind
- 窗口：Tauri 2。Mac 上是 WKWebView，不是 Chromium
- 本机逻辑：Rust。入口 `src-tauri/src/lib.rs`
- 开发地址：`http://127.0.0.1:1420`（`strictPort`）

启动：

```bash
export PATH="$HOME/.cargo/bin:$PATH"
pnpm tauri dev
```

Cargo 源在 `.cargo/config.toml`，走 rsproxy。沙箱可能把 `CARGO_TARGET_DIR` 指到临时目录，开发二进制不一定在 `src-tauri/target`。

关掉窗口后 `tauri dev` 会退出。Vue / CSS 热更新即可。Rust、`src-tauri/permissions`、`src-tauri/capabilities`、`tauri.conf.json` 必须重新编译并重启，热更新不够。

## 代码在哪

| 路径 | 作用 |
|---|---|
| `src/App.vue` | 主窗口：格式、质量、输出、列表 |
| `src/stores/batch.ts` | 列表、输出目录、开始 / 取消 |
| `src/api.ts` | `invoke` 封装。传给 Rust 的对象先用 `plain()` |
| `src/styles.css` | 系统风格和深色变量。深色跟随系统，没有应用内开关 |
| `src-tauri/src/scan.rs` | 扫描、默认输出、路径是否嵌在输入里 |
| `src-tauri/src/pipeline.rs` | 同格式优化和一次转码 |
| `src-tauri/src/batch.rs` | 队列、并发、取消 |
| `src-tauri/src/engines.rs` | 查找并启动引擎 |
| `src-tauri/permissions/app.toml` | 本应用命令白名单 |
| `src-tauri/capabilities/default.json` | 窗口权限，要引用上面的 identifier |

当前命令（新增命令必须同时写进 `permissions/app.toml` 的 `commands.allow`，并重新编译）：

`default_output`、`inspect`、`validate_output`、`load_settings`、`save_settings`、`engine_versions`、`start_batch`、`enqueue_jobs`、`cancel_batch`、`remove_waiting`、`open_output`、`save_copy`、`export_zip`、`read_preview`、`read_thumbnail`。

进度事件是 `file-updated` 和 `batch-finished`。文档里的 `pick_inputs`、`file-started` 没有实现。选文件用对话框插件，在前端完成。

## 已经定下来的行为

- 默认输出格式 WebP，质量「照片」，「再压小」关闭，并发 4（范围 1–8）
- 质量 id 仍是 `photo` / `ui` / `lossless`。界面文案：照片、截图；`lossless` 在保持原格式时显示「画质不变」，换格式时显示「只换格式」
- 「只换格式」：WebP 无损，PNG 不再跑 oxipng，JPEG 用 `cjpeg -quality 100`。不要再禁用 JPEG，也不要自动改回 WebP
- 启动时输出目录是「文档 / Pare Output」。用户没手动选过、且一次只拖入一个文件夹时，改成同级 `名字-pare`。当时输出是空的，要给出可见提示
- 打开输出文件夹用自定义命令 `open_output`：先 `create_dir_all`，再 `tauri_plugin_opener::open_path`。不要改回前端 `openPath`，`opener:default` 不允许打开任意路径
- 引擎查找顺序见 `engines.rs` 的 `candidate_dirs`。`pnpm dist` 会先跑 `scripts/stage-engines.py`，把 macOS 引擎拷进 `src-tauri/resources/engines/` 再打进 `.app`。该目录被 gitignore。不要改 ImageOptim.app 本身
- 动画 GIF 转 JPEG / PNG 失败，文案「动画不能变成单张静图」，除非打开「只用第一帧」。GIF 转 WebP 保留动画
- 透明图转 JPEG 铺白底并警告。JPEG 转 PNG 警告文件通常变大

## 改界面时

Mac 的 WKWebView 会画出系统按钮和滑杆。分段按钮和滑杆要 `-webkit-appearance: none`。选中段用负 margin 铺满，否则蓝底缩在边框里面。

`color-scheme: light dark`，深色写在 `@media (prefers-color-scheme: dark)`。不要假设用户当前是深色。

窗口配置里的 `acceptFirstMouse` 要保留。从访达拖进来之后，第一次点击必须能点到「开始」。

传给 `invoke` 的数据不能是 Vue / Pinia 代理，否则 WebKit 报 `The object can not be cloned.` 继续走 `api.ts` 的 `plain()`。

按钮用 `type="button"`。不要为了统一风格去引入组件库。

程序坞图标：开发模式直接跑二进制时，`set_dock_icon()` 负责。打包后的 `.app` 用 `src-tauri/icons/icon.icns`。

## 测试

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path src-tauri/Cargo.toml
pnpm build
```

改了窗口里能看见的行为，要在桌面窗口里点一遍，不要只看浏览器里的 1420。浏览器没有 Tauri，开始按钮是灰的。
