import { RepositoriesView } from "@/components/repositories/repositories-view";

export default function RepositoriesPage() {
  return (
    <div className="flex flex-col h-full overflow-y-auto p-6 md:p-8">
      <RepositoriesView showHeader={true} />
    </div>
  );
}
