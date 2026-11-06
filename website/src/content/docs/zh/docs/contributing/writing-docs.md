---
title: "编写文档"
description: "这套文档的页面模板、写作规范和源码引用规则。"
sidebar:
  order: 2
---

网站是本仓库的 `website/` 目录，由维护者随 App 同步更新。本页记录网站遵循的约定。若发现页面有误或内容缺失，欢迎提交 issue，或直接修改页面并提交 PR。

这套文档是 `website/src/content/docs/docs/` 下的 Markdown 和 MDX 文件，用 Astro 和 Starlight 构建。那里的每个文件夹对应侧边栏中的一组，页面的 `sidebar.order` 决定它在组中的位置。这些组列在 `website/astro.config.mjs` 中：新建文件夹时需要在那里添加一项，不在文件夹中的页面（例如常见问题）也是如此。

## 页面模板

除总览页（`how-it-works/index.mdx`）外，每个「工作原理」页面都按以下顺序分节，篇幅在 500 到 1200 个单词之间。中文页使用[中文页面](#中文页面)中列出的中文段名。

1. **简单来说**（In short）。开头一个 `:::note[In short]` 框（中文页写 `:::note[简单来说]`），直接回答本页的问题，读者读到这里即可停止。
2. **机制面对的事实**。机制需要处理的客观情况，按事实陈述：各 agent 把文件放在哪里、彼此有何不同。标题按内容命名（「各 agent 如何保存 MCP server」），不写成有待解决的问题。没有这类前提可讲的页面（如审计）省略本段，在「简单来说」中用一句话说明该功能为何存在。
3. **HarnessKit 怎么做**（What HarnessKit does）。按数据流动的顺序介绍步骤：数据来自哪里、写到哪里，以及如何更新和删除。展示用户能看到的文件，改动前后各一份。不要粘贴 Rust 或 TypeScript 代码。
4. **为什么这样设计**（Why we built it this way）。一个 `:::tip` 框，其中包含一个简短的粗体标题、几句理由，以及我们未采用的方案及其代价。
5. **局限**（Limitations）。一个 `:::caution[Limitations]` 框（中文页写 `:::caution[局限]`），列出用户必须了解才能避免出错的行为。只写 HarnessKit 当前的行为和用户的应对办法。用户无法规避的问题不写，修复计划也不写进文档。
6. **相关源码**（Under the hood）。最多 5 条，列出本页背后的源码文件。
7. **延伸阅读**（Related）。用链接卡片指向接下来应阅读的页面。

## 写作规范

文档面向两类读者。一类是 HarnessKit 的用户，读完「HarnessKit 怎么做」即止；另一类是开发者，会读到最后。

- 句子要短，用词平实，一句只讲一件事。
- 中文里技术术语保留英文（见下面的「中文页面」）。中文首页面向新手，因此只把 agent、skill 和 MCP server 保留为英文，也不在一句话中堆砌配置、记忆、规则、hook 和 plugin。首页的产品名词沿用应用中文界面的叫法：扩展、市场、信任评分、每日一帖、工具集。
- 不使用营销词。禁用以下词语：blazing、seamless、powerful、supercharge、unlock、effortless、robust、elevate、revolutionize、frontier。
- 只陈述事实，每一条写之前都到代码中核实。不确定时宁可少写。
- 数量从常量中获取，不要手动输入。`AGENT_COUNT` 和 `AUDIT_RULE_COUNT` 位于 `website/src/data/agents.ts` 中，MDX 页面导入后写作 `{AGENT_COUNT}`。标题中不放数量：目录和标题链接中显示的是常量名，而不是它的值。若页面中有手动输入的数量，`website/src/content/docs-counts.test.ts` 就会失败。
- 用 `website/src/data/agents.ts` 里的显示名，例如 Devin Desktop、DeepSeek Harness、PenguinHarness 和 Qoder CN。
- 颜色只从 `website/src/styles/tokens.css` 中获取。
- 若终端代码块中需要展示命令的输出，请在每条命令前加 `$ `。`$` 会显示为浅色且不会被复制，输出显示为普通文本，复制按钮只复制命令（`website/src/lib/ec-terminal.mjs`）。只包含命令的代码块无需加 `$`。
- 演示视频用 GitHub user-attachments 链接嵌入，并加上 `poster="/docs-posters/<asset id>.jpg"`。在 `website/` 下运行 `node scripts/docs-posters.mjs`（需要 ffmpeg），会把每个视频的第一帧写入 `website/public/docs-posters/`。若有视频缺少封面图，`website/src/content/docs-posters.test.ts` 就会失败。

### 中文页面

中文页存放在 `website/src/content/docs/zh/docs/` 下，路径和文件名与英文页一一对应，发布在 `/zh/docs/<路径>/`。修改英文页时，请同时修改中文页。以下两条原则优先于其他规定：

1. 用应用已有的中文说法，以 `src/lib/i18n/locales/zh/` 里的界面文字为准。
2. 写成地道的中文，不要逐句翻译。事实一条不少，也一条不加。删去翻译腔（进行、通过……来……、对于……来说），也不使用强大、无缝、一站式、赋能这类词。

- **正文术语**。agent、skill、plugin、hook、MCP server、CLI、subagent 保留英文，小写，不加复数；agent 不译作「代理」或「智能体」。其余名词用固定译法：扩展（extension）、工具集（kit）、市场（Marketplace）、信任评分（trust score）、问题（finding）、范围（scope，分全局 / 项目）、每日一帖（Tip of the Day）、Web 模式（web mode）、桌面应用（desktop app）。
- **界面文字**。页面、标签页、按钮、设置项的名称照抄应用的中文界面并加粗，菜单路径用 →，例如**设置 → 智能体路径**。因此 Agents 页写作**智能体**，正文里的 agent 仍写 agent。
- **空格和标点**。中文与英文、数字、行内代码之间加一个半角空格，与全角标点相邻时不加。中文句子用全角标点，并列项之间用顿号，数字用阿拉伯数字。段首的粗体引导语，句号放在粗体之外，写作 `**扫描**。启动时……`；若写作 `**扫描。**启动`，CommonMark 不会将其渲染为粗体。
- **frontmatter**。翻译 `title` 和 `description`；若页面有 `sidebar.label`，也需要翻译，中文侧边栏显示的正是它。其他键原样保留。
- **固定段名**。工作原理页用 `:::note[简单来说]`、按内容命名的开头一段、`## HarnessKit 怎么做`、`:::tip[为什么这样设计]`、`## 局限`、`:::caution[局限]`、`## 相关源码`、`## 延伸阅读`。检查源码引用和数量的测试也会读取中文页。
- **链接**。站内链接指向 `/zh/docs/…`。锚点用中文标题生成的 id：英文转小写，空格变成 `-`，标点去掉，汉字保留，例如 `## 启用和禁用` → `#启用和禁用`。
- **共用的文字**。agent 参考表的说明位于 `website/src/data/agent-reference-zh.ts`；中文常见问题页开头照抄中文首页的问答（`zh.ts`），与英文页照抄 `en.ts` 的方式相同。任何一处不一致，测试都会失败。

## 引用源码

- 「工作原理」和「使用指南」页面只在页面末尾的「相关源码」一节引用源码文件。「参与贡献」下的页面（例如本页）可以在步骤需要时随处写出文件名。
- 路径一律相对仓库根目录，放在反引号里，例如 `crates/hk-core/src/deployer.rs`。文件夹以 `/` 结尾。
- 只写文件，不写行号。行号在下一次改动后就会过时。
- 最多 5 条，每条用一句话说明这个文件做什么。

`website/src/content/cited-paths.test.ts` 会检查 `how-it-works/` 下的每个页面，以及 `reference/`、`contributing/` 下带「相关源码」一节的页面。若引用的路径已从仓库中删除、出现了行号，或超过 5 条，测试都会失败。

## 组件和测试

- **语言**。文档页上的组件（例如 `website/src/components/docs/` 里的 agent 信息卡和 MCP 配置表）按页面路径判断语言（`website/src/lib/docs-lang.ts`），因此中文页与英文页的用法完全相同。
- **样式**。Starlight 的外观定义在 `website/src/styles/starlight.css` 中，基于 `tokens.css` 的配色。
- **截图**。应用截图存放在 `website/src/assets/screens/`。在 `website/` 下运行 `python3 scripts/clean-window-shot.py`（需要 Pillow），可以从桌面截图中裁出窗口，参数说明见脚本开头的注释。
- **新增 agent**。App 新增 agent 后，若网站尚未同步，有三个测试会失败。`website/src/data/agents.test.ts` 将 `AGENTS` 与 App 的 `AGENT_ORDER` 对照，其中的 agent 数量检查还会提醒在 `OverviewLive.tsx` 中为概览窗口补充 `MOCK_EXT_COUNTS` 和 `SCATTER`。`website/src/data/agent-card.test.ts` 将 `CLICK_DURATIONS` 和卡片裁切与 App 的 `agent-card.tsx` 对照。`website/src/data/mascot-ink.test.ts` 要求每个吉祥物都有测量好的偏移：在开发服务器运行时执行 `node scripts/measure-mascot-ink.mjs`，并把输出粘贴到 `mascot-ink.ts` 中。

## 在本地运行网站

网站在 `website/` 下有自己的 `package.json`，需要 Node 22 或更高版本（测试用到了 `fs.globSync`）和 npm。

```sh
cd website
npm ci
npm run dev        # http://localhost:4321
npm test           # vitest, including the citation and agent data checks
npx astro check    # type-checks the pages and components
```

`npm run build` 会执行同样的检查，再把静态网站写入 `website/dist/`。它会先从 GitHub 获取最新 release 的安装包和 star 数；离线时沿用已保存的副本。
