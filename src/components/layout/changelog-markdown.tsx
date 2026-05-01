import Markdown from "react-markdown";

const components = {
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-primary hover:underline"
    >
      {children}
    </a>
  ),
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h4 className="mb-2 text-xs font-medium text-muted-foreground">
      {children}
    </h4>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul className="list-disc pl-4 space-y-1 text-sm text-foreground">
      {children}
    </ul>
  ),
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className="text-sm text-foreground">{children}</p>
  ),
};

/** Renders a release-notes markdown body, or an "improvements" fallback when empty. */
export function ChangelogMarkdown({ body }: { body: string }) {
  if (!body) {
    return (
      <p className="text-sm text-muted-foreground italic">
        Bug fixes and improvements.
      </p>
    );
  }
  return <Markdown components={components}>{body}</Markdown>;
}
