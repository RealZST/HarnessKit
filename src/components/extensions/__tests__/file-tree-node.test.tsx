import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/invoke";
import type { FileEntry } from "@/lib/types";
import { FileTreeNode } from "../file-tree-node";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock("@/lib/invoke", () => ({
  api: {
    readConfigFilePreview: vi.fn(),
    openInSystem: vi.fn(),
  },
}));

const fileEntry = (path: string): FileEntry => ({
  name: path.split("/").pop() ?? path,
  path,
  is_dir: false,
  children: null,
});
function renderTree(entry: FileEntry) {
  return render(
    <FileTreeNode
      entry={entry}
      depth={0}
      expandedPath={entry.path}
      onToggle={vi.fn()}
      dirExpanded={false}
      onToggleDir={vi.fn()}
    />,
  );
}

describe("FilePreview", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("renders the initial truncated preview and offers a full load", async () => {
    vi.mocked(api.readConfigFilePreview).mockResolvedValue(
      "truncated body\n\n... (12 more lines)",
    );
    renderTree(fileEntry("/a/plugin.ts"));

    await waitFor(() => {
      expect(screen.getByText(/truncated body/)).toBeInTheDocument();
    });
    expect(
      screen.getByRole("button", { name: /showFull/ }),
    ).toBeInTheDocument();
    expect(api.readConfigFilePreview).toHaveBeenCalledWith("/a/plugin.ts");
  });

  // A slow "Show full file" fetch for the old file must never overwrite the
  // preview after the tree has navigated to a different file: FilePreview is
  // reused in place when the accordion moves to another entry, so the stale
  // promise can resolve after the path prop changed.
  it("ignores a stale full fetch after the path changed", async () => {
    const user = userEvent.setup();
    let resolveStaleFull: (value: string) => void = () => {};
    vi.mocked(api.readConfigFilePreview).mockImplementation(
      (path: string, maxLines?: number) => {
        if (path === "/a/old.ts" && maxLines !== undefined) {
          return new Promise<string>((resolve) => {
            resolveStaleFull = resolve;
          });
        }
        if (path === "/a/old.ts") {
          return Promise.resolve("truncated body\n\n... (12 more lines)");
        }
        return Promise.resolve("content of new file");
      },
    );

    const utils = renderTree(fileEntry("/a/old.ts"));
    await waitFor(() => {
      expect(screen.getByText(/truncated body/)).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /showFull/ }));
    expect(api.readConfigFilePreview).toHaveBeenCalledWith(
      "/a/old.ts",
      1_000_000,
    );

    // Navigate to a different file: the same node instance now previews the
    // new path, whose load resolves while the old full fetch is still pending.
    utils.rerender(
      <FileTreeNode
        entry={fileEntry("/a/new.ts")}
        depth={0}
        expandedPath="/a/new.ts"
        onToggle={vi.fn()}
        dirExpanded={false}
        onToggleDir={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(screen.getByText(/content of new/)).toBeInTheDocument();
    });

    resolveStaleFull("STALE full content of old");
    await waitFor(() => {
      expect(screen.queryByText(/STALE/)).not.toBeInTheDocument();
    });
    expect(screen.getByText(/content of new/)).toBeInTheDocument();
  });

  it("re-enables the full-load button after navigating to another truncated file", async () => {
    const user = userEvent.setup();
    vi.mocked(api.readConfigFilePreview).mockImplementation(
      (path: string, maxLines?: number) => {
        if (path === "/a/old.ts" && maxLines !== undefined) {
          // Full fetch for the old file never resolves.
          return new Promise<string>(() => {});
        }
        return Promise.resolve("truncated body\n\n... (12 more lines)");
      },
    );

    const utils = renderTree(fileEntry("/a/old.ts"));
    await waitFor(() => {
      expect(screen.getByText(/truncated body/)).toBeInTheDocument();
    });
    await user.click(screen.getByRole("button", { name: /showFull/ }));

    utils.rerender(
      <FileTreeNode
        entry={fileEntry("/a/new.ts")}
        depth={0}
        expandedPath="/a/new.ts"
        onToggle={vi.fn()}
        dirExpanded={false}
        onToggleDir={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /showFull/ })).toBeEnabled();
    });
  });
});
