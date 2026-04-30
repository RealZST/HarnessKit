import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ScopeBadge } from "@/components/shared/scope-badge";

describe("ScopeBadge", () => {
  it("renders Global label with globe icon", () => {
    render(<ScopeBadge scope={{ type: "global" }} />);
    const el = screen.getByLabelText(/scope: global/i);
    expect(el.textContent).toContain("Global");
  });

  it("renders project name", () => {
    render(
      <ScopeBadge scope={{ type: "project", name: "alpha", path: "/p/alpha" }} />,
    );
    const el = screen.getByLabelText(/scope: alpha/i);
    expect(el.textContent).toContain("alpha");
  });
});
