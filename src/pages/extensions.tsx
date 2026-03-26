import { useEffect } from "react";
import { useExtensionStore } from "@/stores/extension-store";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { ExtensionFilters } from "@/components/extensions/extension-filters";

export default function ExtensionsPage() {
  const { extensions, loading, fetch } = useExtensionStore();

  useEffect(() => { fetch(); }, [fetch]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">Extensions</h2>
      </div>
      <ExtensionFilters />
      {loading ? (
        <div className="text-zinc-500">Scanning...</div>
      ) : (
        <ExtensionTable data={extensions} />
      )}
    </div>
  );
}
