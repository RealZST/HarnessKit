import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/invoke";
import { FilePreviewModal } from "../file-preview-modal";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock("@/lib/invoke", () => ({
  api: {
    readConfigFilePreview: vi.fn(),
  },
}));

describe("FilePreviewModal", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("loads + renders preview content from api", async () => {
    vi.mocked(api.readConfigFilePreview).mockResolvedValue("# hello\nworld");
    render(<FilePreviewModal path="/a/CLAUDE.md" onClose={vi.fn()} />);

    await waitFor(() =>
      expect(screen.getByText(/# hello/)).toBeInTheDocument(),
    );
    expect(api.readConfigFilePreview).toHaveBeenCalledWith("/a/CLAUDE.md", 500);
  });

  it("shows error message when api fails", async () => {
    vi.mocked(api.readConfigFilePreview).mockRejectedValue(
      new Error("permission denied"),
    );
    render(<FilePreviewModal path="/a/missing.md" onClose={vi.fn()} />);

    await waitFor(() =>
      expect(screen.getByText(/permission denied/)).toBeInTheDocument(),
    );
  });

  it("Escape key closes modal", async () => {
    vi.mocked(api.readConfigFilePreview).mockResolvedValue("");
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<FilePreviewModal path="/a/CLAUDE.md" onClose={onClose} />);

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});
