import { useEffect, useState } from "react";
import { useRepositoryStore } from "@/stores/repository-store";
import { useIsNew } from "./use-installed-copies";

const STORAGE_KEY = "hk-following-seen-new";

/** Whether a refresh has brought skills the user has not yet seen in
 *  Following. While `viewing` it, every new skill counts as seen, and stays
 *  so afterwards whether or not the user installed it. */
export function useUnseenNew(viewing: boolean): boolean {
  const isNew = useIsNew();
  const repositories = useRepositoryStore((s) => s.repositories);
  const current = repositories.flatMap((repo) =>
    repo.items.filter(isNew).map((item) => `${repo.id}:${item.item_id}`),
  );
  const marks = current.join("\n");
  const [seen, setSeen] = useState(
    () => localStorage.getItem(STORAGE_KEY) ?? "",
  );
  useEffect(() => {
    if (viewing && marks !== seen) {
      localStorage.setItem(STORAGE_KEY, marks);
      setSeen(marks);
    }
  }, [viewing, marks, seen]);

  const seenMarks = new Set(seen.split("\n"));
  return !viewing && current.some((mark) => !seenMarks.has(mark));
}
