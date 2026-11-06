import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { INSTALL_PS1, INSTALL_SH } from "../../data/agents";
import InstallCommand from "./InstallCommand";
import { announceWayUsed } from "./way-used";

const t = {
  kind: "CLI + web mode", kindDesc: "Use it in a terminal or your browser.",
  shells: { sh: "macOS / Linux", ps: "Windows" }, shellsLabel: "Command for",
  next: { sh: "Paste it into your terminal to install. Then run `hk serve`.", ps: "Paste it into PowerShell to install. Then run `hk serve`." },
  copy: "Copy", copyDone: "Copied", viewScript: "View script",
  copyKeys: { mac: "Press ⌘C to copy", other: "Press Ctrl+C to copy" },
};

function mockClipboard(writeText: ReturnType<typeof vi.fn>) {
  Object.assign(navigator, { clipboard: { writeText } });
}
// jsdom has no stylesheet: each shell's line is in the document, so tests look inside one.
const facts = (container: HTMLElement, shell: "sh" | "ps") => container.querySelector<HTMLElement>(`.facts[data-show="${shell}"]`)!;
const copyButton = () => screen.getByRole("button", { name: /^Copy curl/ });
// The visible word beside the icon: the one of the two that isn't hidden.
const word = (container: HTMLElement) => container.querySelector(".cli-words > :not(.off)")?.textContent;

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  delete document.documentElement.dataset.shell;
});

describe("InstallCommand", () => {
  it("heads the command with what it is and what it's for", () => {
    const { container } = render(<InstallCommand t={t} />);
    expect(container.querySelector(".cap-text")).toHaveTextContent("CLI + web mode Use it in a terminal or your browser.");
  });

  it("copies the sh command when <html> has no shell marked, and confirms it on the button and to screen readers", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    mockClipboard(writeText);
    const { container } = render(<InstallCommand t={t} />);
    expect(word(container)).toBe("Copy");
    await userEvent.click(copyButton());
    expect(writeText).toHaveBeenCalledWith(INSTALL_SH);
    expect(word(container)).toBe("Copied");
    expect(await screen.findByRole("status")).toHaveTextContent("Copied");
  });

  it("names the button with its verb first, and the verb once", () => {
    render(<InstallCommand t={t} />);
    // jsdom has no stylesheet, so both shells' commands are in the name here; the drawn word after them is not.
    expect(copyButton()).toHaveAccessibleName(`Copy ${INSTALL_SH}${INSTALL_PS1}`);
  });

  it("follows the shell the platform script marked on <html>", async () => {
    document.documentElement.dataset.shell = "ps";
    const writeText = vi.fn().mockResolvedValue(undefined);
    mockClipboard(writeText);
    render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(writeText).toHaveBeenCalledWith(INSTALL_PS1);
  });

  it("switches the shell on <html> with a labelled pair of toggle tabs", async () => {
    render(<InstallCommand t={t} />);
    const tabs = within(screen.getByRole("group", { name: "Command for" }));
    expect(tabs.getByRole("button", { name: "macOS / Linux" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(tabs.getByRole("button", { name: "Windows" }));
    expect(document.documentElement.dataset.shell).toBe("ps");
    expect(tabs.getByRole("button", { name: "Windows" })).toHaveAttribute("aria-pressed", "true");
    expect(tabs.getByRole("button", { name: "macOS / Linux" })).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps the hero's and the final call to action's copies on the same shell", async () => {
    mockClipboard(vi.fn().mockResolvedValue(undefined));
    render(<InstallCommand t={t} />);
    render(<InstallCommand t={t} />);
    await userEvent.click(screen.getAllByRole("button", { name: "Windows" })[0]);
    for (const button of screen.getAllByRole("button", { name: /^Copy/ })) {
      await userEvent.click(button);
    }
    expect(navigator.clipboard.writeText).toHaveBeenNthCalledWith(1, INSTALL_PS1);
    expect(navigator.clipboard.writeText).toHaveBeenNthCalledWith(2, INSTALL_PS1);
  });

  it("offers a line break after each path separator without changing either command's text", () => {
    const { container } = render(<InstallCommand t={t} />);
    const sh = container.querySelector('code[data-show="sh"]');
    const ps = container.querySelector('code[data-show="ps"]');
    expect(sh?.textContent).toBe(INSTALL_SH);
    expect(ps?.textContent).toBe(INSTALL_PS1);
    expect(sh?.querySelectorAll("wbr").length).toBeGreaterThan(0);
    // Never between the two slashes of "https://"
    expect(sh?.innerHTML).not.toContain("/<wbr>/");
  });

  it("says under the box what to do with each command, the next command as code, and links its script", () => {
    const { container } = render(<InstallCommand t={t} />);
    expect(facts(container, "sh")).toHaveTextContent(/^Paste it into your terminal to install\. Then run hk serve\. · View script/);
    expect(facts(container, "ps")).toHaveTextContent(/^Paste it into PowerShell to install\./);
    expect(facts(container, "sh").querySelector("code")).toHaveTextContent("hk serve");
    const hrefs = screen.getAllByRole("link", { name: "View script" }).map((a) => a.getAttribute("href"));
    expect(hrefs).toEqual([expect.stringMatching(/install\.sh$/), expect.stringMatching(/install\.ps1$/)]);
  });

  it("selects the command and names the keys that copy it when the clipboard rejects", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("denied"));
    mockClipboard(writeText);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    const { container } = render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(writeText).toHaveBeenCalled();
    expect(await screen.findByRole("status")).toHaveTextContent("Press Ctrl+C to copy");
    expect(window.getSelection()?.toString()).toBe(INSTALL_SH);
    expect(word(container)).toBe("Copy");
  });

  it("names ⌘C on a Mac", async () => {
    mockClipboard(vi.fn().mockRejectedValue(new Error("denied")));
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)");
    render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(await screen.findByRole("status")).toHaveTextContent("Press ⌘C to copy");
  });

  it("turns Copied back into Copy after six seconds", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    mockClipboard(vi.fn().mockResolvedValue(undefined));
    const { container } = render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    await act(() => vi.advanceTimersByTimeAsync(5900));
    expect(word(container)).toBe("Copied");
    await act(() => vi.advanceTimersByTimeAsync(100));
    expect(word(container)).toBe("Copy");
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
  });

  it("drops its confirmation when another way is used, or the page is left", async () => {
    mockClipboard(vi.fn().mockResolvedValue(undefined));
    const { container } = render(<InstallCommand t={t} />);
    const button = screen.getByRole("button", { name: /^Copy/ });
    await userEvent.click(button);
    expect(word(container)).toBe("Copied");
    // Its own use doesn't count.
    act(() => announceWayUsed(button));
    expect(word(container)).toBe("Copied");
    act(() => announceWayUsed(document.createElement("a")));
    expect(word(container)).toBe("Copy");

    await userEvent.click(button);
    act(() => { window.dispatchEvent(new Event("pagehide")); });
    expect(word(container)).toBe("Copy");
  });

  it("tells the other ways when it copies", async () => {
    mockClipboard(vi.fn().mockResolvedValue(undefined));
    const used = vi.fn();
    document.addEventListener("hk:way-used", used);
    render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(used).toHaveBeenCalledTimes(1);
    document.removeEventListener("hk:way-used", used);
  });

  it("clears the keys notice after six seconds", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    mockClipboard(vi.fn().mockRejectedValue(new Error("denied")));
    render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(await screen.findByRole("status")).toHaveTextContent(/to copy$/);
    await act(() => vi.advanceTimersByTimeAsync(6000));
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
  });

  it("drops the confirmation when the shell changes", async () => {
    mockClipboard(vi.fn().mockResolvedValue(undefined));
    const { container } = render(<InstallCommand t={t} />);
    await userEvent.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(word(container)).toBe("Copied");
    await userEvent.click(screen.getByRole("button", { name: "Windows" }));
    expect(word(container)).toBe("Copy");
  });
});
