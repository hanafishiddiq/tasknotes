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
  /** What the unlink notice promises stays as it was. */
  keptNoun: "Note" | "Task";
  children: ReactNode;
}

/** One side of a Task–Note relationship: open, unlink with Undo, then a way to add. */
export default function LinkedList<T extends { linkId: string; title: string }>({
  noun, links, icon, onOpen, detail, keptNoun, children,
}: LinkedListProps<T>) {
  return (
    <section className="linked-section"
      aria-label={`${noun} linked to this ${keptNoun === "Note" ? "Task" : "Note"}`}>
      <h4 className="linked-heading">{noun}</h4>
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
          <span>Unlinked “{links.undoable.title}”. The {keptNoun} itself is unchanged.</span>
          <button type="button" className="text-link" aria-label="Undo unlink" disabled={links.busy}
            onClick={() => void links.undo()}>Undo</button>
        </div>
      )}
      {children}
    </section>
  );
}
