// An interactive filter toggle. It is a real button that exposes its selected state via
// aria-pressed, meets the 28px control minimum, and never relies on color alone (pressed also
// changes weight/border).

export interface FilterChipProps {
  label: string;
  pressed: boolean;
  onClick: () => void;
}

export function FilterChip({ label, pressed, onClick }: FilterChipProps) {
  return (
    <button type="button" className="op-chip" aria-pressed={pressed} onClick={onClick}>
      {label}
    </button>
  );
}
