import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import { useProjectStore } from "@/stores/project-store";
import { useScopeStore } from "@/stores/scope-store";

beforeEach(() => {
  useScopeStore.setState({ current: { type: "global" }, hydrated: true });
  useProjectStore.setState({ projects: [], loading: false, loaded: true });
});

const wrap = (ui: React.ReactNode) => <MemoryRouter>{ui}</MemoryRouter>;

describe("ScopeTargetField", () => {
  it("renders a hint (no picker) in single-scope mode", () => {
    useScopeStore.setState({ current: { type: "global" }, hydrated: true });
    render(wrap(<ScopeTargetField value={null} onChange={() => {}} />));
    expect(screen.getByText("Global")).toBeTruthy();
    expect(screen.queryByRole("combobox")).toBeNull();
  });

  it("renders a project hint in project scope", () => {
    useScopeStore.setState({
      current: { type: "project", name: "alpha", path: "/p/alpha" },
      hydrated: true,
    });
    render(wrap(<ScopeTargetField value={null} onChange={() => {}} />));
    expect(screen.getByText("alpha")).toBeTruthy();
  });

  it("renders a required dropdown in All-scopes mode", () => {
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
      loaded: true,
    });
    render(wrap(<ScopeTargetField value={null} onChange={() => {}} />));
    const select = screen.getByLabelText(
      /install to scope/i,
    ) as HTMLSelectElement;
    expect(select).toBeTruthy();
    expect(select.value).toBe("");
  });

  it("calls onChange with selected scope", () => {
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
      loaded: true,
    });
    const onChange = vi.fn();
    render(wrap(<ScopeTargetField value={null} onChange={onChange} />));
    fireEvent.change(screen.getByLabelText(/install to scope/i), {
      target: { value: "global" },
    });
    expect(onChange).toHaveBeenCalledWith({ type: "global" });
  });

  it("renders the picker (not the hint) in single-scope mode when alwaysPick is set", () => {
    useScopeStore.setState({
      current: { type: "project", name: "alpha", path: "/p/alpha" },
      hydrated: true,
    });
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
      loaded: true,
    });
    render(
      wrap(<ScopeTargetField value={null} onChange={() => {}} alwaysPick />),
    );
    const select = screen.getByLabelText(
      /install to scope/i,
    ) as HTMLSelectElement;
    expect(select).toBeTruthy();
    expect(select.value).toBe("");
  });

  it("shows smart-default 'Use X' shortcut when value is null and smartDefault provided", () => {
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
      loaded: true,
    });
    const onChange = vi.fn();
    render(
      wrap(
        <ScopeTargetField
          value={null}
          onChange={onChange}
          smartDefault={{ type: "project", name: "alpha", path: "/p/alpha" }}
        />,
      ),
    );
    fireEvent.click(screen.getByText(/use alpha/i));
    expect(onChange).toHaveBeenCalledWith({
      type: "project",
      name: "alpha",
      path: "/p/alpha",
    });
  });
});
