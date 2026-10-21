# Pare

在 Mac 上把一批图片压小，或一次换成 WebP、JPEG、PNG。原图不改，结果写到新文件夹。图片只在本机处理，不上传，没有账号。

应用标识 `app.pare.desktop`。窗口标题和关于页用 Pare。

## 系统

只提供 macOS、Apple Silicon（M 系列）的安装包。Intel Mac、Windows、iOS、Android 不能用这份包。

## 能做什么

- 拖入文件或文件夹（含子文件夹），也可从系统窗口选择
- 输入：JPEG、PNG、GIF、SVG。其他扩展名留在列表里，标成不支持
- 输出：WebP（默认）、JPEG、PNG，或保持原格式
- 质量：照片、截图；保持原格式时第三档是「画质不变」，换成别的格式时是「只换格式」
- 「再压小」默认关闭，只在「保持原格式」时出现
- 默认写到「文档 / Pare Output」。只拖入一个文件夹、且还没手动选过输出位置时，写到旁边的「文件夹名-pare」
- 保持相对路径。重名用 `-2`、`-3`，不覆盖
- 同格式：只有更小才留下新文件，否则复制原图并标「未变小」
- 换格式：从原图只编码一次，不先做有损压缩
- 处理中可取消。取消不会留下半截文件
- 外观跟随系统浅色 / 深色

更细的范围见 `docs/产品说明.md`、`docs/功能文档.md`、`docs/技术文档.md`。界面文案以当前程序为准：质量档叫「截图」「画质不变 / 只换格式」，不再叫「界面」「无损」。JPEG 也可以选「只换格式」，用最高质量编码，不是像素级无损。

## 许可

见 [LICENSE](LICENSE)。

个人可以非商业地使用、复制和修改本仓库里的 Pare 源代码。禁止商用，包括把 Pare 用于经营或公司业务，出售、出租，放进收费产品或服务，以及用它提供付费处理。

oxipng、pngquant、cwebp 等第三方引擎各自保留原来的许可证。上面的限制只约束 Pare 自己的代码。

## 开发

需要 Node 20 或更新、pnpm、Rust（`rustup`，1.90 或更新）。用户机器不需要这些。

```bash
pnpm install
export PATH="$HOME/.cargo/bin:$PATH"
pnpm tauri dev
```

开发窗口是 `http://127.0.0.1:1420`。关掉窗口后开发进程会退出。只改 Vue 或 CSS 会热更新；改 Rust、权限或 `tauri.conf.json` 会重新编译并重启窗口。

浏览器里打开 1420 只能看界面，不能处理图片。

打包：

```bash
pnpm dist
```

应用在 `src-tauri/target/release/bundle/macos/Pare.app`，磁盘映像在 `src-tauri/target/release/bundle/dmg/`。映像用 `hdiutil` 生成，不经过访达。

Rust 测试：

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path src-tauri/Cargo.toml
```

前端类型检查：`pnpm build`（`vue-tsc` 之后再打前端包）。

## 引擎

`pnpm dist` 会先把引擎收进安装包。收好的文件在 `Pare.app/Contents/Resources/engines/`，不进 Git。

这台电脑需要已经能找到这些命令：ImageOptim 自带的 oxipng、pngquant、zopflipng、jpegtran、jpegoptim、gifsicle、svgcleaner，以及 `PATH` 里的 cwebp、gif2webp、cjpeg。打出来的包给同一芯片的其他 Mac 用，对方不用再装这些工具。

开发时仍按这个顺序查找：`PARE_ENGINES`、程序旁边的 `engines`、安装包里的 `Resources/engines`、本机 ImageOptim、`PATH`、`/opt/homebrew/bin`、`/usr/local/bin`。

SVG 转成位图在进程内用 resvg，不调用外部程序。

不要改 `/Applications/ImageOptim.app`。Pare 只读取它自带的引擎。

## 目录

```text
src/                 Vue 界面
src-tauri/           Rust、窗口配置、权限
src-tauri/src/       扫描、流水线、批处理
engines/manifest.json
docs/                产品、功能、技术说明
```
