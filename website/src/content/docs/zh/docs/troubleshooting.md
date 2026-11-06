---
title: 常见问题排查
description: HarnessKit 的常见问题：出现的原因，以及相应的解决办法。
---

每一条都从你看到的现象入手，再说明原因和解决办法。与故障无关的一般问题，请参阅[常见问题](/zh/docs/faq/)。若这里没有列出你遇到的情况，请[提交 issue](https://github.com/RealZST/HarnessKit/issues/new)，写明你的操作、结果以及所用的版本。

## 安装和启动

### 提示 `hk: command not found`

在 Windows 上，PowerShell 会提示无法将 `hk` 识别为 cmdlet、函数、脚本文件或可运行程序的名称。

**原因**。安装脚本把 `hk` 放在 `~/.local/bin`，Windows 上为 `%USERPROFILE%\.local\bin`。若该文件夹不在 PATH 中，脚本会将其加入：根据你使用的 shell 写入 `~/.zshrc`、`~/.bashrc` 或 `~/.profile`，在 Windows 上写入用户 Path。已打开的终端不会读取这项改动。

**解决办法**。打开一个新终端（在服务器上则退出后重新登录），再运行 `hk --version`。若仍找不到 `hk`，请先确认 `~/.local/bin/hk` 存在，再把该文件夹加入你的 shell 实际读取的启动文件。例如，macOS 终端中的 bash 读取的是 `~/.bash_profile`，而不是 `~/.bashrc`。若 `hk` 是手动下载的，请按[手动下载](/zh/docs/getting-started/web/#手动下载)中的步骤将其加入 PATH。

### 双击 hk.exe 后窗口随即关闭

**原因**。`hk.exe` 是命令行版本，在 release 页面上名为 `hk-windows-x64.exe`。双击启动时，它打印完帮助信息即退出，窗口也随之关闭。

**解决办法**。如需使用桌面应用，请改为下载并运行 `HarnessKit_x.x.x_x64-setup.exe`。如需使用命令行，请打开 PowerShell 或 Windows 终端，在其中运行 `hk`，例如 `hk serve`。两者的区别请参阅[应该下载哪个版本？](/zh/docs/faq/#应该下载哪个版本)。

### 运行安装包前 Windows 弹出警告

打开 `HarnessKit_x.x.x_x64-setup.exe` 时，Windows 显示「Windows 已保护你的电脑」。

**原因**。此警告来自 SmartScreen。安装包尚无代码签名，因此 SmartScreen 无法识别它。

**解决办法**。点击「更多信息」，再点击「仍要运行」。后续步骤请参阅[安装桌面应用](/zh/docs/getting-started/desktop/#windows)。

## Web 模式

### 网页可以打开，但没有数据

页面可以加载，但列表始终为空，所有操作也都失败。

**原因**。`hk serve` 要求访问 token，而浏览器没有为这个地址保存 token。浏览器在你打开完整链接时保存 token，且只对该地址有效：在浏览器看来，`http://localhost:7070` 和 `http://127.0.0.1:7070` 是两个不同的地址，端口不同也算不同地址。换用其他浏览器、使用无痕窗口或清除网站数据后，都不再有这个 token。HarnessKit 没有登录页：不带 token 的请求，服务器一律以「401 Unauthorized」拒绝。

**解决办法**。重新打开 `hk serve` 打印的完整链接，即以 `?token=…` 结尾的链接。每次启动 `hk serve`，token 都保持不变。详见[访问 token](/zh/docs/getting-started/web/#访问-token)。

### `hk serve` 报错「Address already in use」

`hk serve` 打印出链接后随即退出，并报告 `Error: Address already in use` 之类的错误。在 Windows 上，提示为每个套接字地址只允许使用一次。

**原因**。该端口（默认为 7070）已被其他程序监听。常见情况是之前启动的 `hk serve` 仍在运行。

**解决办法**。在另一个 `hk serve` 所在的终端按 Ctrl+C 停止它，或改用其他端口启动当前实例，例如 `hk serve --port 8080`。通过 SSH 连接时，请转发你选择的端口：`ssh -L 8080:localhost:8080 user@your-server`。可用的参数请参阅[端口和地址](/zh/docs/getting-started/web/#端口和地址)。

若被占用的是你本地电脑上的端口，`ssh -L` 会打印 `bind [127.0.0.1]:7070: Address already in use` 之类的警告，然后照常登录，但不会建立隧道。此时链接连接的是你电脑上占用该端口的程序。请改用其他本地端口，例如 `ssh -L 7071:localhost:7070 user@your-server`，再把链接中的 `7070` 改为 `7071`。

## agent

### 未检测到某个 agent

**原因**。只要 agent 的主目录存在（例如 Claude Code 的 `~/.claude`），HarnessKit 就视其为已安装。从未启动过的 agent 可能尚未创建该文件夹。安装在其他位置或用环境变量改过位置的 agent，其文件夹位于别处。桌面应用不读取 shell 中设置的环境变量。

**解决办法**。先启动一次该 agent，再切回 HarnessKit；在 Web 模式下则刷新页面。若 agent 的文件夹在其他位置，请打开**设置 → 智能体路径**，点击 agent 旁的编辑按钮，选择文件夹，再点击**保存**。HarnessKit 查找的是哪个文件夹、各 agent 读取哪些环境变量，请参阅[支持的 agent 和文件位置](/zh/docs/reference/agents/#所有-agent)。

### 未使用的 agent 显示为已安装

**原因**。检测只判断 agent 的主目录是否存在，其他工具创建的文件夹也会被计入。

**解决办法**。打开**设置 → 智能体路径**，点击该 agent 的**已启用**按钮，使其变为**已禁用**。此后，它不会再出现在 HarnessKit 的列表和安装目标中。

### 桌面应用中看不到 `hk list` 列出的 CLI

**原因**。在 macOS 上，桌面应用启动时不读取 shell 的启动文件，因此它的 PATH 中只有系统文件夹。HarnessKit 还会在 `~/.local/bin`、`~/.cargo/bin`、`~/go/bin`、`~/.bun/bin`、`/opt/homebrew/bin` 和 `/usr/local/bin` 中查找。在 nvm、Volta 或 fnm 下用 `npm install -g` 安装的 CLI 不在这些文件夹中。从终端启动的 `hk` 使用该终端的 PATH，因此能找到这些 CLI。

**解决办法**。在终端中运行 `hk serve`，改用网页界面。网页界面显示的 CLI 与 `hk list` 一致。

## 扩展

### 改动在 agent 中未生效

**原因**。agent 在会话开始时加载 skill、MCP server、hook 和 plugin，正在运行的会话仍使用当时加载的内容。因此，HarnessKit 的提示末尾都注明「将在新会话中生效」。

**解决办法**。在 agent 中新建一个会话，或退出 agent 后重新打开。Codex 的 hook 和 Grok Build 的项目设置还需要额外一步，见下面两条。

### 在 HarnessKit 之外的改动未显示

**原因**。HarnessKit 显示的是上一次扫描的结果。桌面应用在启动时扫描，窗口每次重新获得焦点时也会扫描。Web 模式只在页面加载时扫描。

**解决办法**。在 Web 模式下，刷新页面即可。在桌面应用中，切换到其他窗口后再切回即可。

### Codex 的 hook 不运行

**原因**。Codex 只运行你在 Codex 中信任过的 hook。HarnessKit 会把 hook 写入 Codex 的文件，但无法代你信任它。hook 的详情面板中也有提示：「Codex hook 必须先通过 `codex /hooks` 信任后才能运行。」

**解决办法**。启动 Codex，运行它的 `/hooks` 命令，信任该 hook。Codex 的 hook 文件请参阅 [Codex](/zh/docs/reference/agents/#codex)。

### Grok Build 项目中的 hook、MCP server 或 plugin 未加载

**原因**。只有在 Grok Build 中信任了项目文件夹，Grok Build 才会加载该项目的 hook、MCP server 和 plugin。skill 不受此限制。详情面板会在这些条目上显示提示。

**解决办法**。在项目文件夹中打开终端，运行 `grok`，当它询问是否信任该文件夹时选择「是」。Grok Build 的项目文件请参阅 [Grok Build](/zh/docs/reference/agents/#grok-build)。

### 安装到其他 agent 的 hook 从不触发

**原因**。HarnessKit 会转换 hook 的事件名，但 matcher 原样复制，而各个 agent 的工具名称并不相同。Gemini CLI 中匹配 `run_shell_command` 的 hook 原样写入 Claude Code 后，Claude Code 中并没有这个名称的工具。Claude Code 的 prompt hook 安装到其他 agent 后会变成 shell 命令，agent 会尝试把 prompt 文本当作命令运行。Cursor 和 Copilot 的情况相反：HarnessKit 去掉了 matcher，hook 会在每次调用工具时运行。

**解决办法**。在目标 agent 的 hook 文件中找到该 hook，把 matcher 改为该 agent 中对应工具的名称。各 agent 的 hook 文件请参阅[支持的 agent 和文件位置](/zh/docs/reference/agents/)。hook 安装到其他 agent 后还有哪些变化，请参阅[安装 hook](/zh/docs/how-it-works/installing-a-hook/#局限)。

### agent 的配置文件中有注释时安装失败

详情面板只提示「安装到 Copilot 失败」（Copilot 处显示的是出错的 agent）。HarnessKit 中也看不到该文件中的 MCP server 或 hook。

**原因**。大多数 agent 的 JSON 配置文件，HarnessKit 都按严格的 JSON 读写。带注释或末尾有多余逗号的文件（例如 Copilot 使用的 VS Code `mcp.json`）无法解析。因此 HarnessKit 不会列出该文件中的任何内容，也会拒绝写入，以免丢失你的注释。OpenCode 和 OpenClaw 的配置文件例外，HarnessKit 会保留它们的注释。

**解决办法**。删除文件中的注释和末尾多余的逗号，再重新安装。如需查看安装失败的原因，请勾选这一行并使用**安装到智能体**：结果摘要会列出每个失败项及其报错。哪些文件在 HarnessKit 写入后仍保留注释，请参阅[安装 MCP server](/zh/docs/how-it-works/installing-an-mcp-server/#局限)。

## 概览

### 不显示每日一帖

**原因**。每次打开**概览**时，HarnessKit 都会从 `raw.githubusercontent.com` 下载提示并保留一份副本，下载失败时使用该副本。若首次下载就失败，则没有副本，也就没有提示。若 HarnessKit 检测到的 agent 都没有对应的提示，提示卡片也不会显示。部分 agent 目前还没有提示。

**解决办法**。确认你的浏览器或桌面应用能够访问 `raw.githubusercontent.com`，再重新打开**概览**。若你的 agent 还没有提示，可以按[贡献每日一帖和 CLI 条目](/zh/docs/contributing/tips-and-cli-listings/#贡献一条提示)中的步骤添加一条。每天的提示如何选出，请参阅[每日一帖](/zh/docs/how-it-works/tips/)。
