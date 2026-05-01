import { vi } from "vitest";

vi.mock("@/lib/dialog", () => ({
  openDirectoryPicker: vi.fn(),
}));

import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";
import { ScopeSwitcher } from "@/components/layout/scope-switcher";
import { useProjectStore } from "@/stores/project-store";
import { useScopeStore } from "@/stores/scope-store";

beforeEach(() => {
  localStorage.clear();
  useScopeStore.setState({ current: { type: "global" }, hydrated: true });
  useProjectStore.setState({ projects: [], loading: false });
});

const renderSwitcher = () =>
  render(
    <MemoryRouter>
      <ScopeSwitcher />
    </MemoryRouter>,
  );

describe("ScopeSwitcher", () => {
  it("shows current scope label in trigger", () => {
    renderSwitcher();
    expect(
      screen.getByRole("button", { name: /switch scope/i }).textContent,
    ).toContain("Global");
  });

  it("opens dropdown on click", () => {
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    expect(screen.getByRole("listbox")).toBeTruthy();
  });

  it("hides 'All scopes' entry when no projects exist", () => {
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    expect(screen.queryByText(/all scopes/i)).toBeNull();
  });

  it("shows 'All scopes' entry when projects exist", () => {
    useProjectStore.setState({
      projects: [
        {
          id: "alpha",
          name: "alpha",
          path: "/p/alpha",
          created_at: "",
          exists: true,
        },
      ],
      loading: false,
    });
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    expect(screen.getByText(/all scopes/i)).toBeTruthy();
  });

  it("selecting a project updates scope-store", () => {
    useProjectStore.setState({
      projects: [
        {
          id: "alpha",
          name: "alpha",
          path: "/p/alpha",
          created_at: "",
          exists: true,
        },
      ],
      loading: false,
    });
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    fireEvent.click(screen.getByText("alpha"));
    expect(useScopeStore.getState().current).toEqual({
      type: "project",
      name: "alpha",
      path: "/p/alpha",
    });
  });

  it("Escape closes dropdown", () => {
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("clicking outside closes dropdown", () => {
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("ArrowDown moves active option, Enter selects", () => {
    // Start in All-scopes so a transition to Global is observable.
    useScopeStore.setState({ current: { type: "all" }, hydrated: true });
    useProjectStore.setState({
      projects: [
        {
          id: "alpha",
          name: "alpha",
          path: "/p/alpha",
          created_at: "",
          exists: true,
        },
      ],
      loading: false,
    });
    renderSwitcher();
    fireEvent.click(screen.getByRole("button", { name: /switch scope/i }));
    // Initial activeIndex = 0 → "All scopes" (because projects exist).
    // ArrowDown moves to index 1 → Global. Enter selects it.
    fireEvent.keyDown(document, { key: "ArrowDown" });
    fireEvent.keyDown(document, { key: "Enter" });
    expect(useScopeStore.getState().current.type).toBe("global");
  });
});
