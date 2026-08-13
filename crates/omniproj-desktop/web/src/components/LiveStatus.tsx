// Two persistent, visually-hidden live regions. They are always mounted so an announcement
// only changes their text — never remounts a node mid-mutation, which would make some screen
// readers drop the message. Polite carries progress (refresh/save/Undo); assertive carries
// errors that must interrupt.

export interface LiveStatusProps {
  polite: string;
  assertive: string;
}

const srOnly: React.CSSProperties = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0 0 0 0)",
  whiteSpace: "nowrap",
  border: 0,
};

export function LiveStatus({ polite, assertive }: LiveStatusProps) {
  return (
    <>
      <div
        data-testid="live-polite"
        role="status"
        aria-live="polite"
        aria-atomic="true"
        style={srOnly}
      >
        {polite}
      </div>
      <div
        data-testid="live-assertive"
        role="alert"
        aria-live="assertive"
        aria-atomic="true"
        style={srOnly}
      >
        {assertive}
      </div>
    </>
  );
}
