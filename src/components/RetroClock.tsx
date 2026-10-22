interface RetroClockProps {
  time?: string;
  paused?: boolean;
  large?: boolean;
}

export default function RetroClock({ time, paused = false, large = false }: RetroClockProps) {
  return (
    <span
      className={`retro-clock${large ? " retro-clock--large" : ""}${paused ? " is-paused" : ""}`}
      role="timer"
      aria-label={time ?? "00:00"}
    >
      <span className="retro-clock__digits">{time ?? "--:--"}</span>
    </span>
  );
}
