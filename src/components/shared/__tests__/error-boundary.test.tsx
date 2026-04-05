import { describe, it, expect, vi } from "vitest";
import { createElement } from "react";
import { createRoot } from "react-dom/client";
import { act } from "react";
import { ErrorBoundary } from "../error-boundary";

/** A component that always throws during render */
function ThrowingChild(): never {
  throw new Error("Test explosion");
}

describe("ErrorBoundary", () => {
  it("renders fallback UI when a child throws", async () => {
    const container = document.createElement("div");
    document.body.appendChild(container);

    // Suppress expected React error boundary console output
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    await act(async () => {
      const root = createRoot(container);
      root.render(
        createElement(ErrorBoundary, null, createElement(ThrowingChild)),
      );
    });

    // Verify fallback UI is displayed
    expect(container.textContent).toContain("Something went wrong");
    expect(container.textContent).toContain("Test explosion");
    expect(container.textContent).toContain("Reload");

    // Verify the Reload button is present
    const button = container.querySelector("button");
    expect(button).not.toBeNull();
    expect(button?.textContent).toBe("Reload");

    errorSpy.mockRestore();
    document.body.removeChild(container);
  });

  it("renders children when no error occurs", async () => {
    const container = document.createElement("div");
    document.body.appendChild(container);

    await act(async () => {
      const root = createRoot(container);
      root.render(
        createElement(
          ErrorBoundary,
          null,
          createElement("p", null, "All good"),
        ),
      );
    });

    expect(container.textContent).toContain("All good");
    expect(container.textContent).not.toContain("Something went wrong");

    document.body.removeChild(container);
  });
});
