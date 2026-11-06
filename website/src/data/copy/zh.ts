import { AUDIT_RULE_COUNT, REPO } from "../agents";
import { WHICH_ONE_ID, type CopyShape } from "./en";

// Same facts and sources as en.ts; the comments there say where each claim comes from.
// Landing-only convention (the docs follow docs/contributing/writing-docs.md): agent, skill and MCP server stay
// English and singular; the other kinds, configs, memory and rules are not listed in sentences, and are said in plain
// Chinese where one is needed (设置, 指令, 记忆). Product nouns use the App's zh names (扩展, 市场, 信任评分, 安全审计,
// 工具集). EN "setup" is 配置. Links into the docs go to their Chinese pages, under /zh/docs/. Windows' own button
// labels are its zh ones (更多信息, 仍要运行).
export const zh: CopyShape = {
  nav: {
    home: "HarnessKit 首页", primary: "主导航", docs: "文档", github: "GitHub", download: "下载", skip: "跳转到内容",
    theme: { auto: "主题：自动", light: "主题：浅色", dark: "主题：深色" },
  },
  hero: {
    title: "所有 agent，一个家。",
    shotAlt: "HarnessKit 的概览页：每个 agent 和它装的扩展数、每日一帖、agent 最近的改动，以及最近安装的扩展。",
    // The circled phrase stays at six characters or fewer, so it never wraps. "显示在", not "放进": nothing is moved.
    ledeBefore: ["每个 agent 各自装着 skill、MCP\u00a0server、plugin、hook 和 CLI。"],
    ledeLead: "HarnessKit 把它们全部显示在",
    ledeCircled: "一个窗口里，",
    ledeAfter: ["还能一键加给另一个 agent。"],
    // "Web 模式": README.zh-CN's name for it.
    other: "其他安装方式",
    cliWay: "安装 CLI",
    kinds: { app: "桌面应用", cli: "CLI + Web 模式" },
    kindsDesc: { app: "在自己的 Mac 或 Windows 电脑上用。", cli: "在终端或浏览器里用，任何机器都行，远程服务器也可以。" },
    downloadingPhone: {
      mac: "macOS 安装包会存到这台设备上。要安装，请用上面的「发送到我的电脑」把链接发到电脑。",
      windows: "Windows 安装包会存到这台设备上。要安装，请用上面的「发送到我的电脑」把链接发到电脑。",
    },
    mac: { label: "下载 macOS 版", meta: "适用于 Apple Silicon 芯片的 Mac", other: "Intel 芯片？", metaIntel: "适用于 Intel 芯片的 Mac", otherIntel: "Apple Silicon 芯片？", downloading: "正在下载，打开后把 HarnessKit 拖进「应用程序」" },
    // SmartScreen's buttons as Chinese Windows names them, as the FAQ does.
    windows: { label: "下载 Windows 版", meta: "适用于 Windows 10 和 11，64 位", note: "第一次打开时，Windows 可能会弹出提示：点「更多信息」，再点「仍要运行」。", downloading: "正在下载，下载完打开安装程序" },
    cli: {
      shells: { sh: "macOS / Linux", ps: "Windows" }, shellsLabel: "命令适用于",
      next: { sh: "粘贴到终端里装好，再运行 `hk serve`，打开它给出的链接。", ps: "粘贴到 PowerShell 里装好，再运行 `hk serve`，打开它给出的链接。" },
      copy: "复制", copyDone: "已复制", viewScript: "查看脚本",
      copyKeys: { mac: "按 ⌘C 复制", other: "按 Ctrl+C 复制" },
    },
    fineprint: "免费开源 · 不用注册账号",
    available: "免费开源 · 支持 macOS、Windows、Linux，也能在浏览器里用",
    send: { label: "发送到我的电脑", meta: "HarnessKit 在 Mac、Windows 或 Linux 电脑上运行。", copied: "链接已复制" },
  },
  download: {
    title: "下载 HarnessKit",
    description: "下载 HarnessKit 的 macOS 或 Windows 桌面应用，或者安装 CLI，在浏览器里打开同一个窗口。",
    app: { title: "桌面应用", body: "在自己的 Mac 或 Windows 电脑上用。打开它，它会自动找到电脑上已有的 agent。" },
    cli: { title: "CLI + Web 模式", body: "在 Linux、远程服务器或终端里用。一条命令装好 `hk`，再运行 `hk serve`，打开它给出的链接，就能在浏览器里看到同一个窗口。" },
    shotAlt: "HarnessKit 桌面应用的概览页。",
    terminalAlt: "终端里运行 hk serve，它会打印 HarnessKit 网页界面的链接和访问 token。",
  },
  overview: {
    heading: "支持你常用的 agent",
    listLabel: "HarnessKit 支持的 agent",
    hint: "移上去或点一下卡片。", hintTouch: "点一下卡片。", hintKeys: "用方向键看看其他 agent。",
    note: "移上去或点一下，看它们动起来",
    ask: "申请支持其他 agent",
  },
  problem: {
    title: "装得越多，越看不清。",
    body: "skill、MCP server 等扩展分别装在各个 agent 里。同一个扩展常装了好几份。",
    before: "之前",
    after: "有了 HarnessKit",
    columns: { name: "名称", agents: "智能体", score: "评分" },
    beforeAlt: "每个 agent 旁边各有一小堆扩展。同一个扩展出现在好几个 agent 下。",
    afterAlt: "同样的扩展在 HarnessKit 里列成一张表：每种扩展各一个，每个只出现一次，标出装在哪些 agent 上，以及信任评分。",
  },
  principles: [
    { title: "agent 的文件都留在原处。" },
    { title: "不用账号，不上云。" },
    { title: "卸载 HarnessKit，agent 的配置原样保留。" },
  ],
  // zh headings break only at spaces and punctuation (keep-all in global.css), so a long one carries a comma.
  features: {
    unified: { title: "一键把 skill、MCP\u00a0server 或 hook 加到另一个 agent。", body: "看清每个扩展装在哪些 agent 上。加过去时，HarnessKit 自动写成它认得的格式。", link: "/zh/docs/guides/extensions/", linkText: "扩展是怎么工作的" },
    trust: { caption: `每个分数背后，是 ${AUDIT_RULE_COUNT} 项安全检查。`, title: "每个扩展能做什么，一眼看清。", body: "每个扩展都有 0 到 100 的信任评分，扣分原因一条条列出来。", link: "/zh/docs/guides/audit/", linkText: "评分是怎么算的" },
    everywhere: { title: "agent 跑在哪儿，它就跑在哪儿。", body: "可以当 macOS 或 Windows 应用用，也能在浏览器里打开，agent 装在远程服务器上也行。习惯终端的话，`hk` 命令同样能查看状态和启停扩展。", link: "/zh/docs/getting-started/web/", linkText: "在浏览器里运行" },
  },
  // The pictures use the App's zh UI strings (the App's src/lib/i18n/locales/zh), 智能体 included: they show the App
  // as it is.
  // Extension names stay English, as in the Problem block.
  mockLabels: {
    extensions: {
      label: "扩展列表：打开 skill PDF tools，一键加到 Codex。",
      title: "扩展", name: "名称", agents: "智能体", status: "状态", enabled: "已启用", disabled: "已禁用",
      all: "全部", kind: "类型", perms: "权限", score: "评分",
      permNames: { filesystem: "文件系统", network: "网络", shell: "Shell", database: "数据库" },
      on: "已启用", agentsHeading: "智能体", installTo: "安装到智能体", scope: "安装范围：", global: "全局",
      installed: "已安装到 {agent}",
    },
    audit: {
      label: "安全审计列表，展开的是分数最低的 PDF tools：这个 skill 能读你的文件、联网，还能运行命令。",
      title: "安全审计", run: "运行审计", auditing: "审计中...", scanned: "已扫描 {count} 个扩展", lastRun: "上次运行 刚刚",
      readyTitle: "准备审计", readyBody: "扫描扩展的漏洞、危险命令和信任评分。",
      running: "正在运行安全审计...", scanning: "正在扫描扩展中的安全问题。",
      finding: "{count} 项发现", findings: "{count} 项发现", clean: "无问题",
      severity: { High: "高", Medium: "中" },
      rules: { permissionComboRisk: "权限组合风险", dangerousCommands: "危险命令" },
      note: "这个 skill 能读你的文件、联网，还能运行命令",
    },
    everywhere: {
      label: "HarnessKit 的四种用法：Mac 应用、Windows 应用、连着远程机器 my-server 的浏览器标签页，以及终端里的 hk 命令。三个窗口只是外框不同，侧边栏和概览页（连同上面的 agent）都一样。终端里，hk status 列出同样的 agent，hk list 列出 Cursor 上的 skill，接着禁用一个 skill、启用另一个。",
      modes: { label: "HarnessKit 的打开方式", mac: "Mac 应用", win: "Windows 应用", web: "浏览器", cli: "终端" },
      nav: ["概览", "智能体", "扩展", "工具集", "审计", "市场", "设置"],
      overview: {
        agents: "agents", extensions: "extensions",
        kinds: { skill: "skill", mcp: "MCP server", plugin: "plugin", hook: "hook", cli: "CLI" },
        tip: "每日一帖", activity: "智能体活动", installed: "最近安装",
      },
    },
  },
  minorHeading: "还有这些",
  minor: [
    { icon: "package", title: "工具集", body: "把一套配置存下来，在新项目里直接复用。", link: "/zh/docs/guides/kits/" },
    { icon: "store", title: "市场", body: "找新的 skill 和 MCP server，skill 还有第三方安全评分。", link: "/zh/docs/guides/marketplace/" },
    { icon: "folder-tree", title: "按项目配置", body: "每个项目用自己的配置，或者全局共用一套。", link: "/zh/docs/guides/scopes/" },
    { icon: "file-text", title: "指令和记忆", body: "查看每个 agent 读取的指令和记忆。", link: "/zh/docs/guides/agents/" },
    { icon: "refresh-cw", title: "更新检查", body: "检查已安装的 skill 是否有新版本。", link: "/zh/docs/guides/extensions/#检查更新" },
  ],
  faq: {
    title: "常见问题",
    more: { text: "没找到答案？", docs: "查看文档", ask: "在 GitHub 提问" },
    items: [
      { id: WHICH_ONE_ID, q: "应该下载哪个版本？", a: "如果在自己的 Mac 或 Windows 电脑上使用，请下载桌面应用；如果 agent 运行在 Linux 或远程服务器上，请安装命令行版本，它会提供一个链接，在浏览器中打开即可使用同一界面。Windows 安装包暂未签名，首次运行时可能显示安全提示：点击「更多信息」，再点击「仍要运行」。" },
      { q: "HarnessKit 是否免费？", a: "是的。HarnessKit 是基于 Apache-2.0 许可证的免费开源软件，无需注册账号，也没有付费版本。" },
      { q: "HarnessKit 支持哪些语言？", a: "界面支持英文、简体中文和繁体中文。默认跟随系统或浏览器的语言，也可以在「设置 → 语言」中切换，桌面应用和浏览器中均可。" },
      { q: "HarnessKit 会改动我现有的配置吗？", a: "只在你执行操作时才会。HarnessKit 平时只读取各 agent 的文件，用于显示已安装的内容；只有在你安装、更新、启用、禁用或删除内容，或将工具集添加到项目时，才会写入。修改直接写入各 agent 自己的文件，文件保留在原位置，不会移入 HarnessKit 自己的存储，也不会替换为链接。禁用的内容也可以随时重新启用。[了解磁盘上会有哪些改动](/zh/docs/how-it-works/in-place/)。" },
      { q: "我的配置和 API key 会被上传吗？", a: "不会。HarnessKit 在你的电脑上运行，安全审计也在本地完成：你的文件、配置和密钥都不会上传，也不会发送任何使用数据。它只在检查更新、加载市场、下载你安装的内容以及少量其他查询时联网。[查看完整列表](/zh/docs/how-it-works/in-place/#哪些时候会联网)。" },
      { q: "不再使用 HarnessKit 后，agent 还能正常工作吗？", a: "可以。各 agent 直接读取自己的文件，并不依赖 HarnessKit。卸载后，每个 agent 都保持原有配置。只有禁用的内容需要留意：卸载前请先把它们重新启用。" },
      { q: "可以在远程服务器上使用吗？", a: "可以。在服务器上安装后，用自己电脑上的浏览器访问即可。具体步骤请参阅[配置指南](/zh/docs/getting-started/web/#在远程服务器上启动)。" },
      { q: "我使用的 agent 不在支持列表中，怎么办？", a: `请[提交 issue](https://github.com/${REPO}/issues/new)，告诉我们具体是哪个 agent。你也可以自行添加支持，[adapter 开发指南](/zh/docs/contributing/writing-an-adapter/)介绍了具体步骤。` },
    ],
  },
  final: { title: "从你已有的 agent 开始。", body: "打开 HarnessKit，它会自动找到这些 agent。什么都不用搬，也不用配置。" },
  footer: {
    tagline: "免费开源，统一管理你所有的 AI 编程 agent。",
    product: "产品", docs: "文档", community: "社区",
    links: {
      download: "下载", changelog: "更新日志", roadmap: "路线图",
      gettingStarted: "快速开始", agents: "Agent", extensions: "扩展", cli: "命令行", contributing: "参与贡献",
      github: "GitHub", issues: "问题反馈",
    },
    legal: "Apache-2.0 · RealZST 与贡献者出品",
    lang: "English",
  },
};
