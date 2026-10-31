import type { ReactNode } from "react";
import { X } from "lucide-react";
import type { useLinks } from "../lib/use-links";

interface LinkedListProps<T extends { linkId: string; title: string }> {
  /** "Notes" or "Tasks": the other side of the link. */
  noun: "Notes" | "Tasks";
  links: ReturnType<typeof useLinks<T>>;
  icon: ReactNode;
  onOpen: (item: T) => void;
  /** Extra text after the title, such as a Task's work status. */
  detail?: (item: T) => ReactNode;
  children: ReactNode;
}

/** One side of a Task–Note relationship: open, unlink with Undo, then a way to add. */
export default function LinkedList<T extends { linkId: string; title: string }>({
  noun, links, icon, onOpen, detail, children,
}: LinkedListProps<T>) {
  // The unlink notice promises the other side stays as it was.
  const kept = noun === "Notes" ? "Note" : "Task";
  const owner = noun === "Notes" ? "Task" : "Note";
  // On a Task page this sits under the status group's heading; beside a Note, under the page's own.
  const Heading = noun === "Notes" ? "h3" : "h2";
  return (
    <section className="linked-section"
      aria-label={`${noun} linked to this ${owner}`}>
      <Heading className="linked-heading">{noun}</Heading>
      {links.error && <p role="alert" className="inline-error">{links.error}</p>}
      <ul className="linked-items" aria-label={`Linked ${noun}`}>
        {links.items.map((item) => (
          <li key={item.linkId} className="linked-item">
            <button type="button" className="linked-open" onClick={() => onOpen(item)}>
              {icon}
              <span className="linked-title">{item.title}</span>
              {detail?.(item)}
            </button>
            <button type="button" className="linked-unlink" aria-label={`Unlink ${item.title}`}
              disabled={links.busy} onClick={() => void links.unlink(item)}>
              <X size={13} aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
      {links.nextCursor !== null && (
        <button type="button" className="text-link linked-add" onClick={() => void links.loadMore()}>
          Load more linked {noun}
        </button>
      )}
      {links.undoable && (
        <div role="status" aria-label="Unlinked" className="linked-undo">
          <span>Unlinked “{links.undoable.title}”. The {kept} itself is unchanged.</span>
          <button type="button" className="text-link" aria-label="Undo unlink" disabled={links.busy}
            onClick={() => void links.undo()}>Undo</button>
        </div>
      )}
      {children}
    </section>
  );
}
