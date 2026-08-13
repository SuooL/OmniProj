// A neutral observed fact with no container and no status color — branch, SHA, changed-file
// count, relative time. An optional `title` carries the exact source value (e.g. the full
// timestamp) for hover/focus.

export interface FactLabelProps {
  /** Optional field name, e.g. "branch". */
  label?: string;
  value: string;
  title?: string;
}

export function FactLabel({ label, value, title }: FactLabelProps) {
  return (
    <span className="op-fact" title={title}>
      {label ? <span className="op-fact__label">{label} </span> : null}
      {value}
    </span>
  );
}
