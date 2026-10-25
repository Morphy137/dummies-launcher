interface RetroClockProps {
  time?: string;
  paused?: boolean;
  large?: boolean;
}

const SEGMENT_PATHS = {
  a: "M4 0H20L23 3L20 6H4L1 3Z",
  b: "M24 4V18L21 21L18 18V10Z",
  c: "M24 24V38L21 41L18 38V30Z",
  d: "M4 42H20L23 39L20 36H4L1 39Z",
  e: "M0 24L6 30V38L3 41L0 38Z",
  f: "M0 4L6 10V18L3 21L0 18Z",
  g: "M4 18H20L23 21L20 24H4L1 21Z",
};

const DIGIT_SEGMENTS: Record<string, string> = {
  "0": "abcdef",
  "1": "bc",
  "2": "abdeg",
  "3": "abcdg",
  "4": "bcfg",
  "5": "acdfg",
  "6": "acdefg",
  "7": "abc",
  "8": "abcdefg",
  "9": "abcdfg",
  "-": "g",
};

export default function RetroClock({ time, paused = false, large = false }: RetroClockProps) {
  let width = 0;
  const glyphs = Array.from(time ?? "--:--", (character) => {
    const glyph = { character, x: width };
    width += character === ":" ? 12 : 28;
    return glyph;
  });

  return (
    <span
      className={`retro-clock${large ? " retro-clock--large" : ""}${paused ? " is-paused" : ""}`}
      role="timer"
      aria-label={time ?? "00:00"}
    >
      <svg className="retro-clock__display" viewBox={`0 0 ${width - 4} 42`} aria-hidden="true">
        {glyphs.map(({ character, x }) => (
          <g key={x} transform={`translate(${x} 0)`}>
            {character === ":" ? (
              <>
                <rect className="retro-clock__colon" x="3" y="12" width="5" height="5" />
                <rect className="retro-clock__colon" x="3" y="28" width="5" height="5" />
              </>
            ) : (
              Object.entries(SEGMENT_PATHS).map(([segment, path]) => (
                <path
                  key={segment}
                  className={`retro-clock__segment${(DIGIT_SEGMENTS[character] ?? "g").includes(segment) ? " is-lit" : ""}`}
                  d={path}
                />
              ))
            )}
          </g>
        ))}
      </svg>
    </span>
  );
}
