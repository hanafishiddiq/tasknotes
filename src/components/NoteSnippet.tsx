/** A search snippet from the core, its matched words (between U+0002 and U+0003) marked. */
export default function NoteSnippet({ text }: { text: string }) {
  const parts = text.split(/[\u0002\u0003]/);
  return (
    <span className="note-list-snippet">
      {parts.map((part, index) => index % 2 === 1 ? <mark key={index}>{part}</mark> : part)}
    </span>
  );
}
