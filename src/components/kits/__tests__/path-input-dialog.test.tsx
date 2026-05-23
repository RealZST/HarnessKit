import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PathInputDialog } from "../path-input-dialog";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock("@/lib/dialog", () => ({
  openFilePicker: vi.fn(),
  openDirectoryPicker: vi.fn(),
  saveFilePicker: vi.fn(),
}));

vi.mock("@/lib/transport", () => ({
  isDesktop: () => true,
}));

describe("PathInputDialog", () => {
  it("Escape closes the dialog", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <PathInputDialog
        title="Import"
        submitLabel="Go"
        pickerMode="open"
        onSubmit={vi.fn()}
        onClose={onClose}
      />,
    );
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });

  it("submit button disabled until input has content", async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <PathInputDialog
        title="Import"
        submitLabel="Go"
        pickerMode="open"
        onSubmit={onSubmit}
        onClose={vi.fn()}
      />,
    );
    const submit = screen.getByRole("button", { name: "Go" });
    expect(submit).toBeDisabled();

    const input = screen.getByRole("textbox");
    await user.type(input, "/some/path");
    expect(submit).toBeEnabled();
  });

  it("typing a path and clicking submit calls onSubmit + onClose", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <PathInputDialog
        title="Import"
        submitLabel="Go"
        pickerMode="open"
        onSubmit={onSubmit}
        onClose={onClose}
      />,
    );

    await user.type(screen.getByRole("textbox"), "/x/y");
    await user.click(screen.getByRole("button", { name: "Go" }));
    expect(onSubmit).toHaveBeenCalledWith("/x/y");
    expect(onClose).toHaveBeenCalled();
  });

  it("onSubmit throwing surfaces the error message in the dialog", async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error("bad path"));
    const user = userEvent.setup();
    render(
      <PathInputDialog
        title="Import"
        submitLabel="Go"
        pickerMode="open"
        onSubmit={onSubmit}
        onClose={vi.fn()}
      />,
    );
    await user.type(screen.getByRole("textbox"), "/x/y");
    await user.click(screen.getByRole("button", { name: "Go" }));
    expect(screen.getByText("bad path")).toBeInTheDocument();
  });
});
