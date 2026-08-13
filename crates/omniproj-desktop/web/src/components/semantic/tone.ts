// Internal mapping from a semantic status role to its --op-* token trio. Components pick a
// tone from a FIXED enum keyed on the domain value; a tone or raw color is never a public
// prop, so no caller can inject an arbitrary hue. No hex or color role name appears in any
// component source — only these token references.

export type StatusTone = "neutral" | "info" | "success" | "warning" | "danger";

export function toneStyle(tone: StatusTone): React.CSSProperties {
  return {
    color: `var(--op-status-${tone}-fg)`,
    backgroundColor: `var(--op-status-${tone}-bg)`,
    borderColor: `var(--op-status-${tone}-border)`,
  };
}
