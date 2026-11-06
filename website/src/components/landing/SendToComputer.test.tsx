import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import SendToComputer from "./SendToComputer";

const t = { label: "Send to my computer", meta: "Runs on a computer.", copied: "Link copied" };
const pageUrl = () => location.origin + location.pathname;

function stub(api: { share?: unknown; clipboard?: unknown }) {
  Object.defineProperty(navigator, "share", { value: api.share, configurable: true });
  Object.defineProperty(navigator, "clipboard", { value: api.clipboard, configurable: true });
}

afterEach(() => stub({}));

describe("SendToComputer", () => {
  it("opens the share sheet with this page's address and leaves the facts in place", async () => {
    const share = vi.fn().mockResolvedValue(undefined);
    stub({ share });
    render(<SendToComputer t={t} />);
    await userEvent.click(screen.getByRole("button", { name: "Send to my computer" }));
    expect(share).toHaveBeenCalledWith({ title: document.title, url: pageUrl() });
    expect(screen.getByRole("status")).toHaveTextContent("Runs on a computer.");
  });

  it("does nothing more when the visitor closes the share sheet", async () => {
    const writeText = vi.fn();
    stub({ share: vi.fn().mockRejectedValue(new DOMException("closed", "AbortError")), clipboard: { writeText } });
    render(<SendToComputer t={t} />);
    await userEvent.click(screen.getByRole("button"));
    expect(writeText).not.toHaveBeenCalled();
  });

  it("copies the link and says so where there is no share sheet", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    stub({ clipboard: { writeText } });
    render(<SendToComputer t={t} />);
    await userEvent.click(screen.getByRole("button"));
    expect(writeText).toHaveBeenCalledWith(pageUrl());
    expect(await screen.findByRole("status")).toHaveTextContent("Link copied");
  });

  it("copies the link when sharing fails for another reason", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    stub({ share: vi.fn().mockRejectedValue(new DOMException("denied", "NotAllowedError")), clipboard: { writeText } });
    render(<SendToComputer t={t} />);
    await userEvent.click(screen.getByRole("button"));
    expect(writeText).toHaveBeenCalledWith(pageUrl());
  });

  it("shows the link itself when the clipboard rejects too", async () => {
    stub({ clipboard: { writeText: vi.fn().mockRejectedValue(new Error("denied")) } });
    render(<SendToComputer t={t} />);
    await userEvent.click(screen.getByRole("button"));
    expect(await screen.findByRole("status")).toHaveTextContent(pageUrl());
  });

  it("says nothing under the button until something happens, where a nearby line already gives the facts", async () => {
    stub({ share: undefined, clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
    render(<SendToComputer t={t} statusOnly />);
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    await userEvent.click(screen.getByRole("button", { name: t.label }));
    expect(screen.getByRole("status")).toHaveTextContent(t.copied);
  });
});
