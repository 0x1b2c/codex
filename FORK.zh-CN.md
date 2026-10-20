# Codex 个人 fork

这是 [OpenAI Codex](https://github.com/openai/codex) 的个人 fork，由
[0x1b2c](https://github.com/0x1b2c) 维护。

## 功能速览

- **Vim 编辑：** 左侧模式指示器、适用于中文的 word 边界、带 counts 的命令、Visual 编辑和有限的 register 支持，并非完整 Vim 实现。
- **历史导航：** 在 Vim Normal 模式下，用 Ctrl+U/D、Ctrl+B/F 和 `gg`/`G` 浏览对话历史，不改变草稿。
- **返回最新输出：** Ctrl+Option+E（Ctrl+Alt+E）不受草稿编辑模式限制，不再使用 Esc 作为历史导航快捷键。
- **减少打断：** 移除主动用量阈值警告，保留 `/status` 和实际额度错误。
- **可选提示与隔离：** 本地 prompt cache 时间估计，以及让终端会话不连接共享 daemon 的 `tui.no_daemon`。
- **Build 与安装：** `just build` 同时 build 完整 CLI 与 code-mode host；`just install-fork` 先 build，再按原名安装两者。

本 fork 还修改了 wheel 事件的绘制方式，但尚未证实该补丁的独立性能收益。
增加每次滚动距离使用的是上游已有配置，不是 fork 新增功能。

**版本跟随策略：** 只跟随上游最新正式稳定发布版，不跟随 `main` 或预发布版。
相较上游 `main` 存在较大的 commit 差距是正常现象，不能仅凭该差距判断 fork 是否停止维护。
下文记录的基线表明当前 checkout 跟随哪个发布版本。

可以先阅读[功能启用方式](#功能启用方式)或 [Build 与运行](#build-与运行)。
另有 [English version](FORK.md)；patch stack 细节见后文。

## 概述

本 fork 为日常使用提供有针对性的修复和工作流改善。它不是独立产品：Codex 的名称、
许可证和上游署名保持不变。优先保持 patch stack 精简，并降低跟随上游更新的维护成本。

README 除顶部的 fork 提示外，保留上游内容。本文件介绍 fork 的差异；较长的
fork 专用使用指南放在 `docs/fork/`。

## 功能启用方式

以下配置示例均写入 `~/.codex/config.toml`。请合并到已有 `[tui]` section，
不要创建重复 section，并保留无关设置。修改文件后重启 CLI。
`/vim` 与 `/statusline` 可以在运行中的 UI 内修改各自的设置。

| 功能                            | 启用方式与默认状态                                                                                          |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Prompt cache 估计               | 按需启用：向 `tui.status_line` 添加 `"cache-expiry"`，或在 `/statusline` 中选择。默认状态栏不包含此项。     |
| Vim 编辑与 Normal 模式历史导航  | 用 `/vim` 启用 Vim 模式，或设置 `tui.vim_mode_default = true`，让之后启动的会话默认启用。Vim 模式默认关闭。 |
| 返回最新输出                    | Ctrl+Option+E（Ctrl+Alt+E）在主 fullscreen 视图可直接使用，不需要配置或启用 Vim 模式。                      |
| 独立终端会话                    | 按需设置 `tui.no_daemon = true`；默认为 `false`。                                                           |
| 滚轮移动距离                    | 可选的上游配置：`tui.mouse_scroll_speed = 3.0` 增大滚动距离；默认为 `1.0`。                                 |
| 移除用量警告与 wheel 绘制改动   | 在本 fork 中自动生效，无启用开关。绘制改动用于主 fullscreen 视图，不增大滚动距离。                          |
| 本地 build 设置与 lock 文件保护 | `just build` 自动使用，无需 CLI 配置。                                                                      |

主 fullscreen 对话视图默认启用。Inline 或原始输出模式不提供相同的由应用管理的历史导航行为。

## 本 fork 解决的问题

条目按 topic 分支描述问题及解决方式。一个 topic 可以包含多个内聚的提交；
无需为每条命令或每个实现步骤分别建立条目。

### Vim 编辑应当自然地处理中文

使用 `/vim` 切换当前会话的 Vim 模式。若希望之后启动的会话进入 Vim Normal 模式，
可设置上游已有选项：

```toml
[tui]
vim_mode_default = true
```

该设置默认为 `false`。本 fork 扩展 Vim 编辑，不会自动启用 Vim 模式。

正常编辑草稿时，主输入框左下角显示当前 Vim 模式：`-- NORMAL --`、`-- INSERT --`、
`-- VISUAL --`、`-- VISUAL LINE --` 或 `-- REPLACE --`。原有快捷键帮助提示在满足
显示条件且空间足够时移到右下角。窄窗口优先保留模式标签，配置的状态栏使用剩余空间。
共用编辑器的弹窗也采用相同标签，但保留原有布局。

草稿框中的 Vim word motion 现在使用标点和空白边界处理中文，不引入基于词典的分词。
`w` / `b` / `e` 与对应的 `W` / `B` / `E` 区分 word 与按空白划分的 WORD 边界。
水平移动 `h` 和 `l` 保持在当前逻辑行内，word text object 对空白的处理保持一致。

同一 topic 还扩展了现有 Vim 编辑器：带 counts 的移动和编辑、带 counts 的 dot repeat、
重复字符查找、向光标前粘贴、字符与整行 Visual 编辑、合并行、括号匹配、段落移动与
text object，以及在既有未命名 buffer 之外的 yank register `"0`、命名 register
`"a`–`"z` 和黑洞 register `"_`。未实现删除历史 register `"1`–`"9`、大写追加
register 或系统剪贴板 register。还补充了行首导航、`X` / `S`，包括正确处理空行上的 `cc`。

主对话中，Vim Normal 模式使用 Ctrl+U / Ctrl+D 按半页滚动历史，Ctrl+B / Ctrl+F
按整页滚动，与 Page Up / Page Down 一样保留一行重叠。这些命令复用 pager
快捷键；显式启用的 editor 或 app 绑定优先。`gg` 跳到历史开头，必要时加载更早的页面；
`G` 返回最新输出。它们复用 Vim 的 `jump_top` / `jump_bottom` 绑定，即使草稿非空
也用于浏览历史，并保留草稿文字与光标。Insert、Visual 及未完成的编辑命令保留编辑行为；
`h` / `j` / `k` / `l` 仍移动草稿光标。Ctrl+T 在主 fullscreen 视图中切换详细对话显示，
不改变输入焦点。

macOS 上的 Ctrl+Option+E（其他平台为 Ctrl+Alt+E）在主 fullscreen 对话中返回最新输出，
无论草稿框处于 Vim Normal、Insert、Visual 还是非 Vim 模式。它保留草稿、光标和
编辑模式，任务运行时也可安全地重复按。此快捷键可以配置：

```toml
[tui.keymap.global]
jump_to_latest = "ctrl-alt-e"
```

此动作也会出现在 `/keymap` 中，支持常规 keymap 语法，包括多个绑定
（如 `["ctrl-alt-e", "f12"]`）、组合键序列（如 `"ctrl-x e"`），以及使用 `[]` 解绑。
返回最新输出的提示会显示已配置的快捷键。Ctrl+End、Alt+> 和 Vim Normal 模式下的 `G`
仍是独立的替代操作。
普通历史浏览不再使用 Esc 返回底部：Esc 保留既有编辑、取消、prompt 回溯和任务中断职责。
弹窗与对话框仍优先处理自己的输入。

_Topic：`vim-editing`。范围：`wheel-scrolling..vim-editing`。_

### Prompt cache 的时间应当可见

这是可选的状态栏项目，不是默认指示器。运行 `/statusline` 并选择 `cache-expiry`，
或向已有 `[tui].status_line` 数组添加 `"cache-expiry"`。如果尚未配置数组，
以下示例保留三个默认项目，再追加估计值：

```toml
[tui]
status_line = ["model-with-reasoning", "current-dir", "thread-name", "cache-expiry"]
```

如果已有自定义状态栏，请保留原项目并追加 `"cache-expiry"`，不要用此示例替换整个数组。

提交 prompt 后，该项目显示类似 `Cache ~12:18` 的本地时刻。当前估计为提交时间加
30 分钟；它不是倒计时，也不是服务端保留 cache 的确认，不保证下次请求命中 cache。
在运行中的会话尚无可用于估计的提交时间时，该项目不显示。

_Topic：`cache-expiry`。范围：`local-build..cache-expiry`。_

### 用量警告不应打断对话

主动用量阈值警告不再向对话添加消息，也不再增加警告计数。可通过 `/status` 或状态栏
查看用量；草稿框用量提示、实际额度错误、模型切换与额度恢复流程仍保留。

_Topic：`usage-warnings`。范围：`cache-expiry..usage-warnings`。_

### 历史滚动不应为每个 wheel 事件重绘旧帧

在主 fullscreen 视图的对话正文区域，如果已显示 viewport 的几何布局不变，连续 wheel
事件会更新滚动位置，而不再同步绘制中间帧。现有帧调度器绘制累计后的滚动位置。
初始布局、终端 resize 和草稿高度变化仍会先刷新几何布局，再处理 wheel；点击与拖选
保留同步布局刷新。wheel 事件落在可选择的 footer 或 turn-tip 文字上时，也会在滚动
正文前同步刷新这些区域。

_Topic：`wheel-scrolling`。范围：`usage-warnings..wheel-scrolling`。_

### 终端会话可以独立于 app 的 daemon

即使共享 daemon 已存在，终端 TUI 也可以使用自己的 embedded app server。
在 `~/.codex/config.toml` 已有的 `[tui]` section 中设置：

```toml
[tui]
no_daemon = true
```

对于本地 TUI 启动（包括 resume 与 fork），该配置等同于 `--no-daemon`：跳过
daemon 发现和自动启动。默认为 `false`，保留上游行为。配置为 false 时，显式
`--no-daemon` 仍然生效。显式 `--remote` 连接忽略该本地偏好；`--remote` 与
`--no-daemon` 参数同时使用时，仍按既有行为报错。修改配置后重启 CLI。

该偏好不关闭 ChatGPT app 的共享 daemon，不改变 `features.daemon_auto_start`，
不停止已有 daemon，也不改变 remote control。它不新增 daemon 版本兼容检查，
也不解决多个 remote control 连接的争用。

_Topic：`tui-no-daemon`。范围：`vim-editing..tui-no-daemon`。_

### Build 与开发体验

**`local-build`** 提供优化后的 release profile、`just build` 和 `just install-fork`。
Build recipe 同时 build 完整 CLI 与其 code-mode host，不要求安装官方 Codex，
安装前会先 build，见 [Build 与运行](#build-与运行)。

_Topic：`local-build`。范围：`fork-base..local-build`。_

## Patch stack 约定

### 布局

各 topic 在记录的上游基线上组成一条线性 stack。每个 topic 分支指向该 topic 的最后
一个提交，`master` 是集成端点。这些分支标记同一条历史的边界，不是反复合并到集成分支
的独立分支。

当前 stack 为：

```text
rust-v0.163.0
  -> lock-alignment
  -> fork-base
  -> local-build
  -> cache-expiry
  -> usage-warnings
  -> wheel-scrolling
  -> vim-editing
  -> tui-no-daemon
  -> master
```

`fork-base` 包含 fork 提示、本文件和共享维护规则，使后续 topic 可以在引入行为的同一
提交中更新文档。本地会话记录和机器专用设置不属于这一层。

Stack 基于上游 tag `rust-v0.163.0`（`83086188ae0614bd882818ff1aa23f3194962012`）。
`lock-alignment` 是上游之上的第一个补丁：使 lock 文件中的 workspace package
版本与 manifest 一致，不改变外部依赖。Build recipe 的 `--locked` 保护属于 `local-build`。

以上所有 topic ref 都标记同一条线性历史的实际边界。两个 binary 的 build 和安装逻辑
已合并到 `local-build`；`master` 在 `tui-no-daemon` 之后补充集成层文档。
`backup/patch-stack-before-reorder-20261020` 保留首次 stack 整理前的集成端点，
之后 rebase 时应保持该备份不动。首次整理保留 `rust-v0.162.1` 基线，除本文件外没有
改变 tracked 内容。后续上游同步将 stack 迁移至 `rust-v0.163.0`，保留上游变化和全部
本地功能补丁。`backup/before-upstream-0.163.0-20261020` 保留该次同步前的集成端点，
也必须保持不动。

### 添加与维护补丁

- 每个 topic 聚焦一个问题或内聚功能。独立行为改动保留独立提交，不为了让 topic 只有
  一个提交而 squash。仅在有意整理历史时，将纯纠正合入它所纠正的提交。
- 行为变化与本文件的对应更新放在同一提交中。先说明问题，再说明解决方式与边界。
  不把计划中的工作写成已交付功能。
- Fork 专用行为写在这里，必要时补充到 `docs/fork/`，不写进上游 release notes。
  README 的修改仅限 fork 提示。
- Topic 分支公开后，使用 `compare/<previous-topic>...<topic>` 链接各 topic。
  如果每个 topic 都与上游比较，就会同时显示所有下层补丁。优先用分支名而非 commit
  hash，使链接在上游 rebase 后仍可使用。不添加指向未公开或不存在 ref 的链接。
- 常规 follow-up 改动放在集成端点，重排到较早 topic 可以等到上游同步。
  发布或形成共享检查点后，采用追加提交的方式，直到下次有意整理 stack；保留已有
  release tag 不动。

### 更新上游基线

同步前记录旧上游基线、集成端点和 topic ref。重放 stack 后，用新旧补丁范围的
`git range-diff` 核对变化、新增和删除的提交，并验证 topic ref 仍标记预期边界。
Topic ref 随 stack 迁移，backup ref 保持不动。当 backup ref 指向重写范围内时，
不要自动更新 ref，只显式移动预期的 topic ref。

保持 `codex-rs/Cargo.lock` 中的 workspace package 版本与
`codex-rs/Cargo.toml` 中的 `workspace.package.version` 一致。上游 `rust-v0.162.1`
与 `rust-v0.163.0` 发布时修改了 manifest 版本，但 lock 文件中本地 package 版本
仍为 `0.0.0`；本 fork 记录匹配版本，不改变外部依赖。更新上游基线时再次检查版本是否一致。

上游取代某个补丁后，保留该条目并加删除线和说明。在从当前 stack 移除补丁前，先将
原补丁保存在 `fork/superseded/<topic>`。创建或公开这些 tag 遵循维护者的常规 Git 流程。

## 滚动配置

Codex 已支持滚轮速度倍率。默认每个 wheel 事件移动一行文字，即使输入处理及时，
触控板导航也可能显得慢。若希望每次手势滚得更远，可在 `~/.codex/config.toml` 中
设置 `mouse_scroll_speed`；已有 `[tui]` section 时应合并到其中：

```toml
[tui]
mouse_scroll_speed = 3.0
```

该设置请求每个 wheel 事件移动三行，而非一行。重启 CLI 后生效，不需要重新 build。
可以按偏好调整倍率。这是使用上游已有选项调参，fork 不改变默认值。滚动距离与绘制延迟
是不同问题：该设置增大移动距离，不解决处理停顿。

## Build 与运行

在项目根目录运行：

```sh
just install-fork
```

`just build` 启用 Cargo HTML build timing 报告，Cargo 会在日志中输出报告路径。

本地 build 使用 Cargo 的 `--locked` 参数，读取 tracked lock 文件而不改写。
如果 manifest 变化要求更新 lock 文件，应先显式更新并检查差异，再进行 build。

Recipe 准备 V8 后以 `--release` 调用 Cargo，只 build `codex` 和 `codex-code-mode-host`
两个 binary 及其依赖，不 build workspace 中的全部工具。CLI 包含 TUI 和通常的
`resume`、`exec` 等子命令。两个 binary 均来自当前 checkout，不需要已经安装的官方
CLI 或 Host。薄 Python 入口复用上游打包 helper，为所选 target 下载 Codex 发布的
启用 sandbox 的 V8 预编译库和配套 Rust bindings。Helper 使用本 checkout 固定的
hash 验证 release checksum manifest，再验证两个产物；后续 build 也会验证缓存。
首次 build 需要网络，已有有效缓存时除外。已有成对的 `RUSTY_V8_ARCHIVE` /
`RUSTY_V8_SRC_BINDING_PATH` 覆盖和 `V8_FROM_SOURCE` 沿用上游 helper 的行为。
每次只支持 build 一个 target，多个 target 无法共用同一对 V8 产物。

`just build` 的额外参数直接转发给 Cargo。两个 recipe 默认显式选择本机 target
（`host-tuple`），设置了 `CARGO_BUILD_TARGET` 时则使用该值，沿用本 fork 早期
build 的 target-specific 缓存布局。不显式选择 target 时，Cargo 使用另一套缓存布局；
旧缓存并未丢失，但切换布局可能触发大量重新编译。`just build` 的显式 `--target`
参数会覆盖此默认值。

项目的 release profile 保持 `opt-level = 3`，关闭 LTO（`lto = "off"`），使用
`codegen-units = 16`，关闭 debug 信息（`debug = 0`），移除符号（`strip = "symbols"`），
默认关闭 incremental compilation（`incremental = false`）。仅 `codex-tui` 启用
incremental compilation，以额外磁盘占用换取之后本地 TUI rebuild 的时间缩短。
首次 build 必须生成 incremental cache，最终 CLI 编译和链接仍需执行。这些设置减少
日常优化 build 的编译与磁盘开销，可能带来运行时性能取舍。Backtrace 不再包含源文件和
行号。修改 profile 会在下次 build 触发重新编译，不会自动删除已有缓存产物。

`just build` 只 build，不安装或改变已有命令。默认两个 executable 位于
`codex-rs/target/<host-triple>/release/` 下。`CARGO_TARGET_DIR` 或 Cargo 配置
可以改变 target 目录。两个 executable 应放在同一目录，让 CLI 能找到伴随程序。

macOS 和 Linux 上，`just install-fork` 先 build 两个 binary，再安装到
`~/.local/bin`。Build 失败时不改动已安装的命令。安装步骤向 Cargo 查询 target
目录，因此 `CARGO_TARGET_DIR` 和 Cargo 的 `target-dir` 配置对两个步骤均生效，
无需手动指定产物路径。先成功复制两个源文件，才替换命令。已有文件 symlink 被替换为
普通 executable，不改写链接目标；目标为目录时拒绝安装。上游 `just install` 仍仅
准备开发依赖，保持原样。

第一个可选参数选择 build 和安装共同使用的 target triple，第二个选择安装 prefix：

```sh
just install-fork aarch64-apple-darwin
just install-fork aarch64-apple-darwin /tmp/codex-prefix
```

第二条命令安装到 `/tmp/codex-prefix/bin`。如需只改变 prefix 而保留默认本机 target，
第一个参数使用 `host-tuple`。第一个参数是 target，不再是 build 输出目录。

Recipe 不修改 `PATH`。把 `~/.local/bin` 放在其他 Codex 安装路径之前，然后运行
`codex`。命令没有 fork 专用后缀。Windows build 生成对应 `.exe` 文件，应把两个文件
复制到 `PATH` 中的同一目录。

## 许可证与署名

上游 Apache-2.0 许可证与署名声明保持不变。见 [LICENSE](LICENSE) 与 [NOTICE](NOTICE)。
