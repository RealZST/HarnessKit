---
title: 常见问题
description: 关于 HarnessKit、它存储的数据以及运行环境的常见问题。
# source: the first eight questions, and the first paragraph under each, are copy/zh.ts faq.items (the landing's
# plain answers), verbatim; the paragraphs after them are docs-only detail, as in the English FAQ.
---

本页简要回答最常见的问题，并链接到详细说明的页面。若遇到无法使用、修改不生效等情况，请先查看[常见问题排查](/zh/docs/troubleshooting/)，那里从你看到的现象入手，给出解决办法。

## 应该下载哪个版本？

如果在自己的 Mac 或 Windows 电脑上使用，请下载桌面应用；如果 agent 运行在 Linux 或远程服务器上，请安装命令行版本，它会提供一个链接，在浏览器中打开即可使用同一界面。Windows 安装包暂未签名，首次运行时可能显示安全提示：点击「更多信息」，再点击「仍要运行」。

[桌面应用](/zh/docs/getting-started/desktop/)支持 macOS（Apple Silicon 和 Intel）和 64 位的 Windows 10、11，不提供 Linux 版。命令行方式会在 macOS、Linux 或 Windows 上安装 `hk` CLI。运行 `hk serve` 会启动 [Web 模式](/zh/docs/getting-started/web/)并给出一个链接，在浏览器中打开即可使用同一界面。

此提示来自 SmartScreen，原因是安装包没有代码签名。文件的下载量达到一定数量后，SmartScreen 便不再提示。

## HarnessKit 是否免费？

是的。HarnessKit 是基于 Apache-2.0 许可证的免费开源软件，无需注册账号，也没有付费版本。

## HarnessKit 支持哪些语言？

界面支持英文、简体中文和繁体中文。默认跟随系统或浏览器的语言，也可以在「设置 → 语言」中切换，桌面应用和浏览器中均可。

`hk` 命令行的输出仅提供英文。本文档提供英文和简体中文两个版本，可以用页面顶部的 EN / 中文 切换。

## HarnessKit 会改动我现有的配置吗？

只在你执行操作时才会。HarnessKit 平时只读取各 agent 的文件，用于显示已安装的内容；只有在你安装、更新、启用、禁用或删除内容，或将工具集添加到项目时，才会写入。修改直接写入各 agent 自己的文件，文件保留在原位置，不会移入 HarnessKit 自己的存储，也不会替换为链接。禁用的内容也可以随时重新启用。[了解磁盘上会有哪些改动](/zh/docs/how-it-works/in-place/)。

禁用 skill 时，HarnessKit 会在其文件名后加上 `.disabled`，例如 `SKILL.md.disabled`。Hermes 例外：HarnessKit 会把该 skill 加入 `~/.hermes/config.yaml` 的 `skills.disabled` 列表。有些写入会重写整个配置文件，例如 Codex 和 Grok Build 的 `config.toml` 会丢失注释，涉及哪些文件请参阅[安装 MCP server](/zh/docs/how-it-works/installing-an-mcp-server/#局限)。

## 我的配置和 API key 会被上传吗？

不会。HarnessKit 在你的电脑上运行，安全审计也在本地完成：你的文件、配置和密钥都不会上传，也不会发送任何使用数据。它只在检查更新、加载市场、下载你安装的内容以及少量其他查询时联网。[查看完整列表](/zh/docs/how-it-works/in-place/#哪些时候会联网)。

HarnessKit 没有遥测。上述少量查询包括：加载界面字体和每日一帖，以及查询新发现的 skill 的来源。查询来源时，HarnessKit 先在 skills.sh 上搜索，这会发送该 skill 的名称；再对匹配到的仓库运行 `git ls-remote`，它只读取最新的 commit，不下载仓库。安全审计的规则是在本机运行的文本匹配，因此离线时也能审计。

## 不再使用 HarnessKit 后，agent 还能正常工作吗？

可以。各 agent 直接读取自己的文件，并不依赖 HarnessKit。卸载后，每个 agent 都保持原有配置。只有禁用的内容需要留意：卸载前请先把它们重新启用。

在没有自带禁用开关的 agent 上，禁用的 MCP server 或 hook 只保存在 `~/.harnesskit/metadata.db` 中，删除该文件后将无法恢复。禁用的 skill 会一直保留 `.disabled` 后缀，以改名方式禁用的 plugin（改名的是 manifest 或 plugin 文件）也是如此，需要手动改回原名。[卸载](/zh/docs/getting-started/update-and-uninstall/#卸载)一节说明了卸载前需要重新启用和删除哪些内容。

## 可以在远程服务器上使用吗？

可以。在服务器上安装后，用自己电脑上的浏览器访问即可。具体步骤请参阅[配置指南](/zh/docs/getting-started/web/#在远程服务器上启动)。

步骤概述：在服务器上安装，用 `ssh -L 7070:localhost:7070` 转发端口，运行 `hk serve`，再在本地浏览器中打开它给出的链接。集群上的步骤相同。Web 模式的功能与桌面应用一致，仅缺少 [Web 模式不支持的功能](/zh/docs/getting-started/web/#web-模式不支持的功能)中列出的几项。

## 我使用的 agent 不在支持列表中，怎么办？

请[提交 issue](https://github.com/RealZST/HarnessKit/issues/new)，告诉我们具体是哪个 agent。你也可以自行添加支持，[adapter 开发指南](/zh/docs/contributing/writing-an-adapter/)介绍了具体步骤。

每个 agent 都对应一个用 Rust 编写的 adapter。

## agent 安装在其他位置时，HarnessKit 能否找到？

能。打开**设置 → 智能体路径**，点击 agent 旁的编辑按钮，选择它实际使用的文件夹，再点击**保存**。桌面应用不读取 shell 中设置的环境变量，因此若用 `GROK_HOME` 等变量移动了 agent 文件夹，也需要在此处设置路径。[支持的 agent 和文件位置](/zh/docs/reference/agents/)列出了 HarnessKit 默认在哪个文件夹中查找各个 agent。

HarnessKit 尚无 adapter 的 agent，目前无法自行添加。可以[提交 issue](https://github.com/RealZST/HarnessKit/issues/new) 申请支持。

## 双击 hk.exe 后窗口随即关闭，是什么原因？

你打开的是命令行版本。`hk.exe`（下载时名为 `hk-windows-x64.exe`）需要在终端中运行。双击启动时，它打印完帮助信息后，窗口会随即关闭。

如需使用桌面应用，请改为下载 `HarnessKit_x.x.x_x64-setup.exe`。如需使用命令行版本，请打开 PowerShell 或 Windows 终端，运行 `hk serve`，再打开它给出的链接。若文件是手动下载的，请先按[手动下载](/zh/docs/getting-started/web/#手动下载)中的步骤将其加入 PATH。

## 能在一个窗口中管理多台机器吗？

暂不支持。请在每台机器上分别运行 `hk serve`，并在浏览器中为每台机器各打开一个标签页。

每个标签页和侧边栏都会显示机器名称：默认为主机名，也可以用 `hk serve --name` 自定义。通过 SSH 连接时，请为每台机器使用不同的本地端口，浏览器才会为每台机器分别保存 token。具体步骤请参阅[在远程服务器上启动](/zh/docs/getting-started/web/#在远程服务器上启动)。

## 可以在 Docker 或其他容器中运行 HarnessKit 吗？

目前还没有官方镜像。Linux 版的 `hk` 二进制文件为静态链接，可在任何 Linux 容器中运行，无需额外的库。HarnessKit 只能访问容器内的 agent 文件夹：请将需要它管理的文件夹挂载到容器中；若要在删除容器后保留数据库，也请挂载 `~/.harnesskit`。如需从容器外部打开网页界面，请用 `hk serve --host 0.0.0.0` 启动，并映射端口。从市场或 Git 安装 skill，以及检查和应用更新，都需要容器中有 `git`。

## HarnessKit 的数据存储在哪里？

存储在主目录下的 `~/.harnesskit/` 中：

- `metadata.db` 是一个 SQLite 数据库，存储 HarnessKit 记录的扩展信息：安装来源、审计结果、你的项目和设置，以及它禁用的 MCP server 和 hook 的快照。
- `metadata.db.backup-vN` 是数据库的完整副本，每次升级数据库结构之前生成一份。
- `kits/` 存放你保存的工具集。
- `web-token` 存放 [Web 模式](/zh/docs/getting-started/web/)的访问 token。

有些偏好不在此文件夹中，而是保存在应用的浏览器存储中：语言、主题、智能体的可见性、是否看过欢迎页，以及缓存的每日一帖。桌面应用单独保存一份；在 Web 模式下，每个浏览器各保存一份。

删除此文件夹后，HarnessKit 的数据将恢复到初始状态，agent 的文件不受影响，但上述偏好不会重置。仅存在于数据库中的已禁用 MCP server 和 hook 也会一并丢失，因此删除前请先将它们重新启用。详见[原地管理文件](/zh/docs/how-it-works/in-place/#harnesskit-存储的内容)。

## 多个用户可以在同一台机器上使用 HarnessKit 吗？

可以。每个用户都有自己的 `~/.harnesskit`，因此数据库、工具集和 Web 模式 token 也各自独立。HarnessKit 管理的是运行它的用户主目录中的 agent。在共享服务器上，每个人应使用不同的 `--port` 分别运行 `hk serve`，并保持 token 启用：这台机器上的其他用户也能访问 `127.0.0.1`，只有 token 能阻止他们访问。
