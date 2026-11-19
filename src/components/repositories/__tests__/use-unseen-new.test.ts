import { renderHook } from "@testing-library/react";
import { expect, it } from "vitest";
import type { RepositoryWithItems } from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { useUnseenNew } from "../use-unseen-new";

const repoWithNew = (...names: string[]) =>
  [
    {
      id: "r1",
      items: names.map((name) => ({
        item_id: name,
        is_new: true,
        ignored: false,
        installed: [],
      })),
    },
  ] as unknown as RepositoryWithItems[];

it("counts new skills as seen once Following was opened, until a refresh brings others", () => {
  useRepositoryStore.setState({ repositories: repoWithNew("pdf") });
  const { result, rerender } = renderHook(
    ({ viewing }) => useUnseenNew(viewing),
    { initialProps: { viewing: false } },
  );
  expect(result.current).toBe(true);

  rerender({ viewing: true });
  rerender({ viewing: false });
  expect(result.current).toBe(false);

  useRepositoryStore.setState({ repositories: repoWithNew("pdf", "docx") });
  rerender({ viewing: false });
  expect(result.current).toBe(true);
});
