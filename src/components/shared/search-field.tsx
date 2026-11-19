import { Search, X } from "lucide-react";

/** A search box with a clear button, sized to share a toolbar row. */
export function SearchField({
  value,
  onChange,
  placeholder,
  ariaLabel,
  clearLabel,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  ariaLabel: string;
  clearLabel: string;
}) {
  return (
    <div className="relative min-w-48 max-w-md flex-1">
      <Search
        size={14}
        className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
      />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onChange("");
        }}
        placeholder={placeholder}
        aria-label={ariaLabel}
        className="w-full rounded-lg border border-border bg-card py-2 pl-9 pr-8 text-sm placeholder:text-muted-foreground transition-[background-color,border-color,box-shadow] duration-200 focus:border-ring focus:bg-background focus:shadow-md focus:outline-none"
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          aria-label={clearLabel}
          className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
}
