import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { useUpdateStore } from "@/stores/update-store";
import { useWebUpdateStore } from "@/stores/web-update-store";
import { UpdateDialog } from "../update-dialog";
import { WebUpdateDialog } from "../web-update-dialog";

const available = { version: "9.9.9", body: "notes" };

describe.each([
  {
    name: "UpdateDialog",
    Dialog: UpdateDialog,
    open: () => useUpdateStore.setState({ showChangelog: true, available }),
    isOpen: () => useUpdateStore.getState().showChangelog,
  },
  {
    name: "WebUpdateDialog",
    Dialog: WebUpdateDialog,
    open: () => useWebUpdateStore.setState({ showDialog: true, available }),
    isOpen: () => useWebUpdateStore.getState().showDialog,
  },
])("$name keyboard handling", ({ Dialog, open, isOpen }) => {
  beforeEach(() => open());

  it("moves focus into the dialog and keeps Tab cycling inside it", async () => {
    const user = userEvent.setup();
    render(<Dialog />);
    const dialog = screen.getByRole("dialog");
    expect(dialog.contains(document.activeElement)).toBe(true);

    const focusable = dialog.querySelectorAll<HTMLElement>("button, a[href]");
    focusable[focusable.length - 1].focus();
    await user.tab();
    expect(document.activeElement).toBe(focusable[0]);
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<Dialog />);
    await user.keyboard("{Escape}");
    expect(isOpen()).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
