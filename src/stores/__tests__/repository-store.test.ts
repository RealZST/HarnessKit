import { expect, it, vi } from "vitest";
import type { RepositoryWithItems } from "@/lib/types";
import { useRepositoryStore } from "../repository-store";

const api = vi.hoisted(() => ({ listRepositories: vi.fn() }));
vi.mock("@/lib/invoke", () => ({ api }));

it("keeps the newer list when an older fetch returns after it", async () => {
  const lists: ((list: RepositoryWithItems[]) => void)[] = [];
  api.listRepositories.mockImplementation(
    () => new Promise((resolve) => lists.push(resolve)),
  );
  const { fetch } = useRepositoryStore.getState();
  const older = fetch();
  const newer = fetch();

  lists[1]([{ id: "new" } as RepositoryWithItems]);
  await newer;
  lists[0]([]);
  await older;

  expect(useRepositoryStore.getState().repositories).toEqual([{ id: "new" }]);
});
