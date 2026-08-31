// An event verb paired with its time, e.g. "Completed · 3 days ago". Neutral; the exact
// instant lives in `title`. Used by the commitment history rail (Task 11) and defined
// here as part of the shared semantic grammar.

export interface ActivityStampProps {
  verb: string;
  /** Relative time text, e.g. "3 days ago". */
  text: string;
  /** The exact source timestamp for hover/focus. */
  title?: string;
}

export function ActivityStamp({ verb, text, title }: ActivityStampProps) {
  return (
    <span className="op-stamp" title={title}>
      {verb} · {text}
    </span>
  );
}
