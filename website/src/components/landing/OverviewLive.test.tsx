import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AGENTS, AGENT_COUNT } from "../../data/agents";
import { cardName, clickDuration } from "../../data/agent-card";
import { en } from "../../data/copy/en";
import OverviewLive, { SCATTER } from "./OverviewLive";

const t = en.overview;
// The card in view: the one lit, its mascot playing. "" when none is.
const shownCard = (container: HTMLElement) => container.querySelector(".ov-card.on")?.textContent ?? "";
// A card's name is its visible text: the App's label.
const nameOf = (a: (typeof AGENTS)[number]) => cardName(a);
const card = (fullName: string) => screen.getByRole("button", { name: nameOf(AGENTS.find((a) => a.name === fullName)!) });
const mascotOf = (name: string) => card(name).querySelector("[class^='mascot-']")!;

describe("OverviewLive", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("has a scatter start for every agent", () => {
    const ids = AGENTS.map((a) => a.id);
    expect(ids.filter((id) => !(id in SCATTER))).toEqual([]);
  });

  it("renders one card per agent in AGENTS order, named by its visible text, in a labelled list", () => {
    render(<OverviewLive t={t} />);
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(AGENT_COUNT);
    // No aria-label: the name is what the card shows (WCAG 2.5.3, label in name).
    expect(buttons.filter((b) => b.hasAttribute("aria-label"))).toEqual([]);
    expect(buttons.map((b) => b.textContent)).toEqual(AGENTS.map(nameOf));
    expect(screen.getByRole("list", { name: t.listLabel })).toBeInTheDocument();
  });

  it("shows the App's card label and no count", () => {
    render(<OverviewLive t={t} />);
    expect(card("DeepSeek Harness").textContent).toBe("DeepSeek");
    expect(card("Claude Code").textContent).toBe("Claude Code");
  });

  it("describes the list with the arrow-key hint, which shows under the cards", () => {
    const { container } = render(<OverviewLive t={t} />);
    expect(screen.getByRole("list", { name: t.listLabel })).toHaveAccessibleDescription(t.hintKeys);
    expect(container.querySelector(".ov-caption .ov-keys")).toHaveTextContent(t.hintKeys);
  });

  it("lights the hovered card until the pointer leaves the block, over one line with every hint", () => {
    const { container } = render(<OverviewLive t={t} />);
    expect(container.querySelector(".ov-caption")).toHaveTextContent(`${t.hint}${t.hintTouch}${t.hintKeys}`);
    fireEvent.mouseEnter(card("Pi"));
    expect(shownCard(container)).toBe("Pi");
    fireEvent.mouseLeave(container.querySelector(".ov")!);
    expect(shownCard(container)).toBe("");
  });

  it("plays the hover loop on the shown card only", () => {
    render(<OverviewLive t={t} />);
    fireEvent.mouseEnter(card("Pi"));
    expect(mascotOf("Pi")).toHaveClass("is-animated");
    expect(mascotOf("Codex")).not.toHaveClass("is-animated");
  });

  it("plays a click for that mascot's duration; another click meanwhile does not extend it", () => {
    vi.useFakeTimers();
    render(<OverviewLive t={t} />);
    fireEvent.click(card("Claude Code"));
    expect(mascotOf("Claude Code")).toHaveClass("is-clicked");
    expect(mascotOf("Codex")).not.toHaveClass("is-clicked");
    act(() => { vi.advanceTimersByTime(clickDuration("claude") - 100); });
    fireEvent.click(card("Claude Code"));
    act(() => { vi.advanceTimersByTime(100); });
    expect(mascotOf("Claude Code")).not.toHaveClass("is-clicked");
    // Codex has no entry of its own, so it takes the App's default.
    fireEvent.click(card("Codex"));
    act(() => { vi.advanceTimersByTime(clickDuration("codex") - 1); });
    expect(mascotOf("Codex")).toHaveClass("is-clicked");
    act(() => { vi.advanceTimersByTime(1); });
    expect(mascotOf("Codex")).not.toHaveClass("is-clicked");
  });

  it("follows keyboard focus, and Escape stops the card but keeps focus", async () => {
    const { container } = render(<OverviewLive t={t} />);
    await userEvent.tab();
    expect(card(AGENTS[0].name)).toHaveFocus();
    expect(shownCard(container)).toBe(AGENTS[0].name);
    expect(mascotOf(AGENTS[0].name)).toHaveClass("is-animated");
    await userEvent.keyboard("{Escape}");
    expect(shownCard(container)).toBe("");
    expect(card(AGENTS[0].name)).toHaveFocus();
  });

  it("is one Tab stop: the arrow keys, Home and End move between the cards and play the one in focus", async () => {
    render(<><OverviewLive t={t} /><a href="/next">next</a></>);
    expect(screen.getAllByRole("button").filter((b) => b.tabIndex === 0)).toEqual([card(AGENTS[0].name)]);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}");
    expect(card(AGENTS[1].name)).toHaveFocus();
    expect(mascotOf(AGENTS[1].name)).toHaveClass("is-animated");
    expect(mascotOf(AGENTS[0].name)).not.toHaveClass("is-animated");
    await userEvent.keyboard("{End}");
    expect(card(AGENTS[AGENT_COUNT - 1].name)).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(card(AGENTS[AGENT_COUNT - 1].name)).toHaveFocus();
    await userEvent.keyboard("{Home}{ArrowLeft}");
    expect(card(AGENTS[0].name)).toHaveFocus();
    // Tab leaves the grid, and Shift+Tab comes back to the card last in focus.
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    await userEvent.tab();
    expect(screen.getByRole("link", { name: "next" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(card(AGENTS[2].name)).toHaveFocus();
  });

  it("moves Up and Down to the nearest card in the row above or below", async () => {
    // Three cards a row, 100px apart, 17 cards: rows of 3, and the last row's two cards centred. The second card is
    // lifted 2px, as the card in view is.
    const place = (i: number) => {
      const row = Math.floor(i / 3);
      const inRow = row === 5 ? 2 : 3;
      const left = (300 - inRow * 100) / 2 + (i % 3) * 100;
      const top = row * 100 - (i === 1 ? 2 : 0);
      return { top, left, width: 90, height: 90, right: left + 90, bottom: top + 90, x: left, y: top, toJSON() {} } as DOMRect;
    };
    render(<OverviewLive t={t} />);
    const buttons = screen.getAllByRole("button");
    buttons.forEach((b, i) => vi.spyOn(b, "getBoundingClientRect").mockReturnValue(place(i)));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}{ArrowDown}");
    expect(buttons[4]).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}{ArrowUp}");
    expect(buttons[1]).toHaveFocus();
    // From the middle of a full row into the two-card row: the nearer of the two.
    await userEvent.keyboard("{End}{ArrowUp}{ArrowDown}");
    expect(buttons[AGENT_COUNT - 1]).toHaveFocus();
  });

  it("while the keys drive the cards, shows the focused one over a resting pointer's, until the pointer moves", async () => {
    const { container } = render(<OverviewLive t={t} />);
    const list = screen.getByRole("list");
    fireEvent.mouseEnter(card("Pi"));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}");
    expect(list).toHaveAttribute("data-keys");
    expect(shownCard(container)).toBe(AGENTS[1].name);
    expect(card("Pi")).not.toHaveClass("on");
    // A still pointer's event (a scroll under it) changes nothing; a move hands the cards back to it.
    fireEvent.pointerMove(card("Pi"), { movementX: 0, movementY: 0 });
    expect(list).toHaveAttribute("data-keys");
    fireEvent.pointerMove(card("Pi"), { movementX: 3, movementY: 0 });
    expect(list).not.toHaveAttribute("data-keys");
    expect(shownCard(container)).toBe("Pi");
  });

  it("does not hold a card after a mouse click once the pointer has left", async () => {
    const { container } = render(<OverviewLive t={t} />);
    await userEvent.click(card("Pi"));
    expect(shownCard(container)).toBe("Pi");
    fireEvent.mouseLeave(container.querySelector(".ov")!);
    expect(shownCard(container)).toBe("");
    expect(mascotOf("Pi")).not.toHaveClass("is-animated");
  });

  it("stops the card when focus leaves the list", async () => {
    const { container } = render(<><OverviewLive t={t} /><a href="/next">next</a></>);
    await userEvent.tab();
    expect(shownCard(container)).toBe(AGENTS[0].name);
    act(() => screen.getByRole("link", { name: "next" }).focus());
    expect(shownCard(container)).toBe("");
  });

  it("gathers once the window is in view, only when the section asked for it", () => {
    const observers: { callback: IntersectionObserverCallback; disconnected: boolean }[] = [];
    vi.stubGlobal("IntersectionObserver", class {
      entry: (typeof observers)[number];
      constructor(callback: IntersectionObserverCallback) { this.entry = { callback, disconnected: false }; observers.push(this.entry); }
      observe() {}
      disconnect() { this.entry.disconnected = true; }
    });
    const visible = (isIntersecting: boolean) => act(() => observers[0].callback([{ isIntersecting } as IntersectionObserverEntry], {} as IntersectionObserver));

    const { unmount } = render(<OverviewLive t={t} />);
    expect(observers).toHaveLength(0);
    unmount();

    const section = document.createElement("section");
    section.dataset.gather = "";
    const { container } = render(<OverviewLive t={t} />, { container: document.body.appendChild(section) });
    const grid = container.querySelector(".ov-grid")!;
    visible(false);
    expect(grid).not.toHaveClass("is-gathered");
    visible(true);
    expect(grid).toHaveClass("is-gathered");
    expect(observers[0].disconnected).toBe(true);
  });
});
