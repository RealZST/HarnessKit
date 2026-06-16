import { useEffect, useState } from "react";
import { isDesktop, transport } from "@/lib/transport";

export interface ServerInfo {
  nodeName: string;
}

// Cache the result for the lifetime of the page so the fetch runs once,
// regardless of how many components read it.
let cached: ServerInfo | null = null;

/**
 * Identity of the node serving this web UI (hostname or `--name`). Returns null
 * in desktop mode or until the fetch resolves. Also reflects the node name into
 * the document title so browser tabs for different remote nodes are
 * distinguishable.
 */
export function useServerInfo(): ServerInfo | null {
  const [info, setInfo] = useState<ServerInfo | null>(cached);

  useEffect(() => {
    // Node identity only matters in web mode; desktop has a single local window.
    if (isDesktop() || cached) return;
    let active = true;
    transport<{ node_name: string }>("server_info")
      .then((res) => {
        if (!active) return;
        cached = { nodeName: res.node_name };
        setInfo(cached);
        if (cached.nodeName) {
          document.title = `HarnessKit · ${cached.nodeName}`;
        }
      })
      .catch(() => {
        /* non-fatal: the header simply omits the node label */
      });
    return () => {
      active = false;
    };
  }, []);

  return info;
}
