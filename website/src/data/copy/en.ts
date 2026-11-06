import { AUDIT_RULE_COUNT, REPO } from "../agents";

/** The FAQ item that says which way in to pick (Faq.astro). */
export const WHICH_ONE_ID = "which-one";

export const en = {
  nav: {
    home: "HarnessKit home", primary: "Primary", docs: "Docs", github: "GitHub", download: "Download", skip: "Skip to content",
    // The theme toggle's name for each state; the state words are Starlight's theme select's.
    theme: { auto: "Theme: Auto", light: "Theme: Light", dark: "Theme: Dark" },
  },
  hero: {
    title: "One home for every agent.",
    // The App's Overview page under the ways in (Hero.astro), as the repo's screenshot shows it.
    shotAlt: "HarnessKit's Overview page: every agent with its number of extensions, a tip of the day, recent agent activity and recently installed extensions.",
    // The problem, then what you do about it. The first line names all five kinds HarnessKit manages, what sets it
    // apart from a skill manager; the last says what a click does with them, without naming kinds again (the user's
    // call: the Install to Agent tiles, src/components/extensions/extension-detail.tsx, take every kind but plugins,
    // INSTALLABLE_KINDS in src/lib/agent-capabilities.ts, and the hero is no place for that). "Another agent", not
    // "your other agents": one agent per click.
    // A line each, so no line ends on a stray few words. The pen circles what HarnessKit does, at the end of its line:
    // `ledeLead`, then `ledeCircled`. The full stop sits inside the circle: outside it, the circle's side padding
    // leaves the dot floating. Keep the circled phrase short (2–4 words), so it never wraps.
    ledeBefore: ["Each agent keeps its own skills, MCP\u00a0servers, plugins, hooks and CLIs."],
    ledeLead: "HarnessKit shows them all",
    ledeCircled: "in one window.",
    ledeAfter: ["One click adds them to another agent."],
    // Two buttons under the lede, on the hero and the final call to action (GetButtons.astro): the visitor's
    // platform's download (`mac.label`, `windows.label`), on Linux `cliWay` (the download page's CLI section), on a
    // phone or tablet `send.label`; then `other`, the download page, which has every way in.
    other: "Other ways to install",
    cliWay: "Install the CLI",
    // The ways in on the download page (InstallOptions.astro): the desktop app's two installers side by side, each
    // with one line of facts, and the command. The command installs the `hk` CLI, which is also web mode (README
    // "Web Mode": the same binary). Builds: .github/workflows/release.yml (macOS arm64 + x64 DMGs, one x64 NSIS
    // installer); platform names: README "Getting Started". `kinds` names each way where it is offered.
    kinds: { app: "Desktop app", cli: "CLI + web mode" },
    kindsDesc: { app: "On your own Mac or Windows computer.", cli: "Use it in a terminal or your browser, on any machine." },
    // After a download tile is tapped on a phone or tablet: the installer only runs on a computer.
    downloadingPhone: {
      mac: "The macOS installer saves to this device. To install, send the link to your computer with the button above.",
      windows: "The Windows installer saves to this device. To install, send the link to your computer with the button above.",
    },
    // The macOS way downloads the Apple Silicon DMG, or the Intel one where the browser says the Mac has an Intel chip;
    // `other` links the other chip's DMG. `meta` / `other` go with Apple Silicon, `metaIntel` / `otherIntel` with Intel.
    // `downloading` replaces the facts for a few seconds after a click, for either chip: the next step, from README
    // "Desktop App" (open the DMG, drag HarnessKit to Applications; run the Windows installer).
    mac: { label: "Download for macOS", meta: "For Apple Silicon Macs", other: "Intel Mac?", metaIntel: "For Intel Macs", otherIntel: "Apple Silicon Mac?", downloading: "Downloading. Open it and drag HarnessKit to Applications." },
    // `note`, shown to Windows visitors only: the installer isn't code-signed, so SmartScreen warns on first run
    // (README "Desktop App", FAQ "Does it work on Windows?"). "May": SmartScreen goes by each file's download reputation.
    windows: { label: "Download for Windows", meta: "For Windows 10 and 11, 64-bit", note: "Windows may warn you the first time. Click More info, then Run anyway.", downloading: "Downloading. Open the installer to set it up." },
    // The command's box. The scripts install the `hk` CLI (install.sh: macOS, Linux x64 / ARM64; install.ps1: Windows),
    // not the desktop app; `hk serve` opens the same UI in a browser, from a remote machine over SSH too (README "Web
    // Mode"). Written for a beginner: no "CLI", "headless" or "SSH". `shells` are the box's tabs, a command each, which
    // `shellsLabel` names for screen readers. `next` is the line under the box: install.ps1 runs in PowerShell, not
    // Command Prompt, and `hk serve` prints the link to open (crates/hk-web/src/lib.rs). `copy` and `copyDone` are the
    // word beside the copy icon, before and after.
    cli: {
      shells: { sh: "macOS / Linux", ps: "Windows" }, shellsLabel: "Command for",
      next: { sh: "Paste it into your terminal to install. Then run `hk serve` and open the link it prints.", ps: "Paste it into PowerShell to install. Then run `hk serve` and open the link it prints." },
      copy: "Copy", copyDone: "Copied", viewScript: "View script",
      // Where the clipboard is out of reach, the box selects the command and says which keys copy it.
      copyKeys: { mac: "Press ⌘C to copy", other: "Press Ctrl+C to copy" },
    },
    fineprint: "Free and open source · No account needed",
    // The line under the two buttons: the one way in a visitor sees is theirs, so this says, quietly, that there are
    // others (as Claude Code's and Zed's pages do under their download).
    available: "Free and open source · Runs on macOS, Windows, Linux and in your browser",
    // Phones and tablets can't run the App, so their first action sends the page to a computer: the share sheet, or
    // a copied link where there is none.
    send: { label: "Send to my computer", meta: "HarnessKit runs on a Mac, Windows or Linux computer.", copied: "Link copied" },
  },
  // The download page (DownloadPage.astro): every way in, a section each, the desktop app and the CLI as equals.
  // Its lede is the hero's fine print and version. `hk serve` prints a link to open (crates/hk-web/src/lib.rs); it
  // doesn't open the browser itself.
  download: {
    title: "Download HarnessKit",
    description: "Download the HarnessKit desktop app for macOS or Windows, or install the CLI and open the same window in your browser.",
    app: { title: "Desktop app", body: "For your own Mac or Windows computer. Open it, and it finds the agents already there." },
    cli: { title: "CLI + web mode", body: "For Linux, a remote server or the terminal. One command installs `hk`. Then `hk serve` prints a link that opens the same window in your browser." },
    shotAlt: "The HarnessKit desktop app's Overview page.",
    terminalAlt: "A terminal running hk serve. It prints the link to HarnessKit's web UI, with its access token.",
  },
  // The agents under the hero (OverviewLive): the App's agent cards, under a heading of their own.
  // No count in the heading: agents keep joining, and the cards under it show them all.
  overview: {
    heading: "Works with the agents you use",
    listLabel: "Agents HarnessKit supports",
    // One hint shows under the cards: `hint` where the pointer can hover, `hintTouch` on touch screens, `hintKeys` in
    // their place while a card has keyboard focus (it also describes the list).
    hint: "Hover or click a card.", hintTouch: "Tap a card.", hintKeys: "Arrow keys move between them.",
    // The hand-drawn note by the window, beside its arrow, on wide screens: it stands in for `hint` there, which only
    // screen readers then get.
    note: "hover or click to see them move",
    // Under the cards: opens a new GitHub issue, to ask for an agent that isn't here.
    ask: "Request another agent",
  },
  // The agents and extensions in the picture are samples: MOCK_GATHER in components/landing/mocks/mock-data.ts.
  problem: {
    // The title names the problem as a contrast; the body states what the picture shows: one extension of each kind,
    // and PDF tools in three piles (gather-data.test.ts checks both). The body
    // names the two kinds beginners know and leaves the rest to the picture. A two-sentence title would take a line
    // per sentence on wide screens (Problem.astro).
    title: "The more you install, the less you see.",
    body: "Each agent installs its own skills, MCP servers and other extensions. The same extension ends up in several places.",
    // The picture's label: it turns from the first to the second as the scroll gathers the piles; without motion,
    // each labels its own state.
    before: "Before",
    after: "With HarnessKit",
    // The list's columns: the App's Extensions table's Name, Agent and Score (the score is the Trust Score).
    columns: { name: "Name", agents: "Agent", score: "Score" },
    beforeAlt: "Each agent with its own loose pile of extensions. The same ones show up under several agents.",
    afterAlt: "The same extensions in one HarnessKit list: a skill, an MCP server, a plugin, a hook and a CLI, each once, with the agents that have it and a trust score.",
  },
  // Titles only, one row under the picture. The evidence lives in the FAQ (files in place, going online).
  // "Keep their setup": docs how-it-works/index.mdx; items you turned off stay off (how-it-works/in-place.mdx).
  // The page's one "No X, no Y" line is here.
  principles: [
    { title: "Your agents’ files stay put." },
    { title: "No account, no cloud." },
    { title: "Uninstall HarnessKit and your agents keep their setup." },
  ],
  features: {
    // Plugins are not deployed cross-agent (crates/hk-core/src/service.rs refuses them), so the sentence names skills
    // and MCP servers, two of the kinds that are. HarnessKit converts the formats (docs guides/extensions "Install to
    // another agent").
    // The no-break space keeps "add it" together, so the heading breaks after the comma.
    unified: { title: "Add a skill, MCP\u00a0server or hook to another agent in one\u00a0click.",
    body: "See which agents have each extension. HarnessKit writes it the way the other agent expects.", link: "/docs/guides/extensions/", linkText: "How extensions work" },
    // Every finding lowers the score (crates/hk-core/src/auditor/mod.rs) and is listed under the extension. Its location
    // is a file, with a line when the rule matched one, or the extension's name when there is no file (MCP servers, CLIs).
    // `caption` sits under the mock: the App's rule count (crates/hk-core/src/auditor/rules.rs all_rules()), which the
    // App's Audit page calls "security checks".
    trust: { caption: `${AUDIT_RULE_COUNT} safety checks behind every score.`, title: "Know what each extension can do.", body: "Each extension gets a Trust Score from 0 to 100, and you can see exactly what lowered it.", link: "/docs/guides/audit/", linkText: "How scores work" },
    // A browser via `hk serve`, on the agents' machine (README "Web Mode"). The link goes to web mode, the part the
    // download buttons above don't cover.
    everywhere: { title: "Runs where your agents run.", body: "Use it as a Mac or Windows app, or open it in your browser, even when your agents live on a remote server, or stay in the terminal with the `hk` command.", link: "/docs/getting-started/web/", linkText: "Run it in your browser" },
  },
  // The three feature pictures. Their text is the App's own UI strings (src/lib/i18n/locales/en: extensions, audit,
  // navigation); `label` is each picture's accessible name, since screen readers skip the picture itself. The sample
  // extensions are MOCK_EXTENSIONS and MOCK_AUDIT in components/landing/mocks/mock-data.ts.
  mockLabels: {
    extensions: {
      label: "The Extensions list: PDF tools, a skill, is opened and added to Codex in one click.",
      title: "Extensions", name: "Name", agents: "Agent", status: "Status", enabled: "enabled", disabled: "disabled",
      // The App's kind filter's first chip and its table headers; Score for its "Audit" column, as the onboarding has it.
      all: "All", kind: "Kind", perms: "Permissions", score: "Score",
      // Each permission icon's name on hover (the App's permissions.*).
      permNames: { filesystem: "File System", network: "Network", shell: "Shell", database: "Database" },
      // The detail panel (the App's detail.enabled, detail.agents, detail.installToAgent, scope.installToScopeColon,
      // scope.global). The headings show in capitals, as the App sets them.
      on: "Enabled", agentsHeading: "Agents", installTo: "Install to Agent", scope: "Install to scope:", global: "Global",
      // The App's toast, without its second sentence ("Takes effect in new sessions").
      installed: "Installed to {agent}",
    },
    audit: {
      label: "The Security Audit list, with the lowest score open: PDF tools, a skill that can read your files, go online and run commands.",
      title: "Security Audit", run: "Run Audit", auditing: "Auditing...", scanned: "{count} extensions scanned",
      // The App's "Last run {time}" with its "Just now", in lower case mid-sentence.
      lastRun: "Last run just now",
      // The page before its first audit (empty.ready*) and while one runs (loadingState).
      readyTitle: "Ready to audit", readyBody: "Scan your extensions for vulnerabilities, dangerous commands, and trust scores.",
      running: "Running security audit...", scanning: "Scanning your extensions for security issues.",
      finding: "{count} finding", findings: "{count} findings", clean: "Clean",
      severity: { High: "High", Medium: "Medium" },
      rules: { permissionComboRisk: "Permission Combination Risk", dangerousCommands: "Dangerous Commands" },
      // The hand-drawn note by the open row: PDF tools' permissions (filesystem, network, shell) in plain words.
      note: "this skill can read your files, go online and run commands",
    },
    everywhere: {
      label: "HarnessKit four ways: as a Mac app, a Windows app, a browser tab connected to a remote machine called my-server, and the hk command in a terminal. The three windows differ only in their frame: the sidebar and the Overview page with its agents look the same. In the terminal, hk status lists the same agents, hk list shows the skills on Cursor, then one skill is disabled and another enabled.",
      // The switch above the picture; `label` names the group for screen readers.
      modes: { label: "Open HarnessKit as", mac: "Mac app", win: "Windows app", web: "Browser", cli: "Terminal" },
      // The App's sidebar, top to bottom; Settings sits apart at the bottom.
      nav: ["Overview", "Agents", "Extensions", "Kits", "Audit", "Marketplace", "Settings"],
      // The Overview page (the App's overview locale): its status line's words, the kinds under it (plural), and
      // its sections' headings.
      overview: {
        agents: "agents", extensions: "extensions",
        kinds: { skill: "skills", mcp: "MCP servers", plugin: "plugins", hook: "hooks", cli: "CLIs" },
        tip: "Tip of the day", activity: "Agent activity", installed: "Recently installed",
      },
    },
  },
  minorHeading: "A few more things",
  minor: [
    // Marketplace: skills.sh and Smithery list skills and MCP servers; third-party audit scores are for skills only
    // (docs guides/marketplace). Instructions & memory: configs, rules and memory files, which the Agents page lists.
    { icon: "package", title: "Kits", body: "Save a setup and reuse it in a new project.", link: "/docs/guides/kits/" },
    { icon: "store", title: "Marketplace", body: "Find new skills and MCP servers, with third-party safety scores for skills.", link: "/docs/guides/marketplace/" },
    { icon: "folder-tree", title: "Per-project setups", body: "Give a project its own setup, or use one everywhere.", link: "/docs/guides/scopes/" },
    { icon: "file-text", title: "Instructions & memory", body: "See the instructions and memory each agent reads.", link: "/docs/guides/agents/" },
    { icon: "refresh-cw", title: "Update checks", body: "Check every skill you installed for a newer version.", link: "/docs/guides/extensions/#check-for-updates" },
  ],
  faq: {
    title: "Frequently asked questions",
    // Under the heading, for what the list doesn't answer: the docs, and a new GitHub issue (the repo has no Discussions).
    more: { text: "Can’t find an answer?", docs: "Read the docs", ask: "Ask on GitHub" },
    // Inline markup: `code` and [text](href); see src/lib/inline-code.ts. Plain answers: the docs FAQ opens each
    // question with the same answer, then gives the technical detail (faq-sync.test.ts checks). The questions are the
    // ones a newcomer asks before trusting a tool with their agents' setup (which build, cost, language, will it change or
    // upload anything, can I leave; Windows is answered under which build), in full sentences and the App's terms (disable,
    // not "turn off").
    items: [
      // The install row's "Which one?" opens this one (`id`). Builds: macOS and Windows desktop apps; the CLI on macOS,
      // Linux and Windows, whose `hk serve` prints a link to the same UI (README "Getting Started"). The Windows warning is
      // SmartScreen's: the installer isn't code-signed (v1.15.0 release notes, README "Desktop App (macOS / Windows)").
      { id: WHICH_ONE_ID, q: "Which version should I download?", a: "If you use your agents on your own Mac or Windows computer, download the desktop app. If they run on Linux or a remote server, install the command-line version: it prints a link that opens the same interface in your browser. The Windows installer isn’t code-signed yet, so Windows may show a warning the first time you run it: select More info, then Run anyway." },
      { q: "Is HarnessKit free?", a: "Yes. HarnessKit is free and open source under the Apache-2.0 license. It requires no account and has no paid plan." },
      // The App's languages (src/lib/i18n/index.ts SUPPORTED_LANGUAGES, all fully translated), "Follow System" first:
      // detectSystemLanguage reads navigator.languages, the OS language in the desktop app and the browser's in web
      // mode. The picker is Settings → Language (src/pages/settings.tsx), in web mode too.
      { q: "What languages is HarnessKit available in?", a: "The interface is available in English, Simplified Chinese and Traditional Chinese. It follows your system or browser language by default, and you can change it in Settings → Language, in the desktop app or in your browser." },
      // HarnessKit writes on install, update, enable, disable and delete, and when a Kit is added to a project. It never
      // moves an agent's files into its own storage or swaps them for links (hk-core creates no symlinks); installing to
      // another agent copies, so "nothing is copied" would be wrong (docs how-it-works/in-place.mdx).
      { q: "Will HarnessKit change my existing setup?", a: "Only when you ask it to. HarnessKit reads your agents’ files to show what’s installed, and writes to them only when you install, update, enable, disable or delete something, or add a Kit to a project. Changes go into each agent’s own files, which stay where they are. HarnessKit doesn’t move them into its own storage or replace them with links, and anything you disable can be re-enabled. [See what changes on disk](/docs/how-it-works/in-place/)." },
      // No telemetry; the audit's rules run locally and offline (docs how-it-works/audit.mdx); the lookups are fonts, the
      // Tip of the Day and skills.sh (docs how-it-works/in-place.mdx "What goes online").
      { q: "Do my configs or API keys leave my computer?", a: "No. HarnessKit runs on your computer, and so does its security audit: your files, configs and keys are never uploaded, and no usage data is sent. It goes online only to check for updates, load the Marketplace, download what you install and make a few small lookups. [See the full list](/docs/how-it-works/in-place/#what-goes-online)." },
      // Agents read their own files with or without HarnessKit; disabled snapshots live only in metadata.db and disabled
      // skills keep SKILL.md.disabled (docs how-it-works/in-place.mdx "Limitations").
      { q: "If I stop using HarnessKit, will my agents still work?", a: "Yes. Your agents read their own files directly and never depend on HarnessKit. Remove it, and every agent keeps the setup it had. Only what you disabled needs attention: re-enable it before you uninstall." },
      // Install, forward a port over SSH, run `hk serve` (docs getting-started/web.mdx "Start on a remote server").
      { q: "Can I use HarnessKit on a remote server?", a: "Yes. Install it on the server and open it in the browser on your own computer. [The setup guide](/docs/getting-started/web/#start-on-a-remote-server) walks through the steps." },
      { q: "My agent isn’t supported. What can I do?", a: `[Open an issue](https://github.com/${REPO}/issues/new) and tell us which agent you use. You can also add support yourself: [the adapter guide](/docs/contributing/writing-an-adapter/) explains how.` },
    ],
  },
  // First launch detects the installed agents and scans them (docs getting-started/desktop.md).
  final: { title: "Start with the agents you already have.", body: "Open HarnessKit and it finds them. There’s nothing to move or set up." },
  footer: {
    // Under the wordmark. README's subtitle: "A free, open-source app to manage all your AI coding agents".
    tagline: "A free, open-source app for all your AI coding agents.",
    product: "Product", docs: "Docs", community: "Community",
    links: {
      download: "Download", changelog: "Changelog", roadmap: "Roadmap",
      gettingStarted: "Getting started", agents: "Agents", extensions: "Extensions", cli: "CLI", contributing: "Contributing",
      github: "GitHub", issues: "Issues",
    },
    legal: "Apache-2.0 · Made by RealZST and contributors",
    lang: "简体中文",
  },
} as const;
export type Copy = typeof en;

// Same keys as `en`, any string values. Arrays keep their element shape.
type Loose<T> = T extends string
  ? string
  : T extends readonly (infer U)[]
    ? readonly Loose<U>[]
    : { [K in keyof T]: Loose<T[K]> };
/** What every locale provides, and what landing components accept, so they render `zh` as well as `en`. */
export type CopyShape = Loose<Copy>;
