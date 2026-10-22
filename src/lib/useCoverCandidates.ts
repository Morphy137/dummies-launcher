import { useEffect, useState } from "react";
import { useCachedCover } from "./useCachedCover";

interface CandidateState {
  index: number;
  failed: boolean;
}

const candidateStates = new Map<string, CandidateState>();

function stateKey(candidates: string[]): string {
  return candidates.join("\n");
}

function initialState(key: string, candidateCount: number): CandidateState {
  const saved = candidateStates.get(key);
  if (!saved || saved.index >= candidateCount) {
    return { index: 0, failed: false };
  }
  return saved;
}

export function useCoverCandidates(candidates: string[]) {
  const key = stateKey(candidates);
  const candidateCount = candidates.length;
  const [state, setState] = useState<CandidateState>(() => initialState(key, candidateCount));

  useEffect(() => {
    setState(initialState(key, candidateCount));
  }, [key, candidateCount]);

  const cover = candidates[state.index];
  const src = useCachedCover(state.failed ? undefined : cover);

  const handleError = () => {
    setState((current) => {
      const next =
        current.index + 1 < candidates.length
          ? { index: current.index + 1, failed: false }
          : { index: current.index, failed: true };
      candidateStates.set(key, next);
      return next;
    });
  };

  return {
    src,
    failed: state.failed,
    handleError,
  };
}
