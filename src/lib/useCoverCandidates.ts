import { useCallback, useEffect, useState } from "react";
import { useCachedCover } from "./useCachedCover";

interface CandidateState {
  index: number;
  failed: boolean;
}

const candidateStates = new Map<string, CandidateState & { at: number }>();

function stateKey(candidates: string[]): string {
  return candidates.join("\n");
}

function initialState(key: string, candidateCount: number): CandidateState {
  const saved = candidateStates.get(key);
  if (!saved || saved.index >= candidateCount || Date.now() > saved.at) {
    return { index: 0, failed: candidateCount === 0 };
  }
  return saved;
}

function remember(key: string, state: CandidateState, lifetime: number) {
  candidateStates.delete(key);
  candidateStates.set(key, { ...state, at: Date.now() + lifetime });
  if (candidateStates.size > 256) {
    const oldest = candidateStates.keys().next().value;
    if (oldest) candidateStates.delete(oldest);
  }
}

export function useCoverCandidates(candidates: string[]) {
  const key = stateKey(candidates);
  const candidateCount = candidates.length;
  const [selection, setSelection] = useState(() => ({ key, ...initialState(key, candidateCount) }));
  const state = selection.key === key ? selection : initialState(key, candidateCount);

  const cover = candidates[state.index];
  const { src, failed: loadFailed } = useCachedCover(state.failed ? undefined : cover);

  const handleError = useCallback(() => {
    const next =
      state.index + 1 < candidateCount
        ? { index: state.index + 1, failed: false }
        : { index: state.index, failed: true };
    remember(key, next, 5 * 60_000);
    setSelection({ key, ...next });
  }, [key, state.index, candidateCount]);

  const handleLoad = useCallback(() => {
    remember(key, { index: state.index, failed: false }, 30 * 60_000);
  }, [key, state.index]);

  useEffect(() => {
    if (loadFailed) handleError();
  }, [loadFailed, handleError]);

  return {
    src,
    failed: state.failed,
    handleError,
    handleLoad,
  };
}
