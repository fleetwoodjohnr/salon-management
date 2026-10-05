// Tracks forms with unsaved changes so navigation and window close can warn first.
import { useEffect, useSyncExternalStore } from "react";

const dirty = new Set<string>();
const listeners = new Set<() => void>();
let snapshot = 0;

function emit() {
  snapshot++;
  listeners.forEach((l) => l());
}

export function setDirty(id: string, isDirty: boolean) {
  const had = dirty.has(id);
  if (isDirty && !had) dirty.add(id);
  else if (!isDirty && had) dirty.delete(id);
  else return;
  emit();
}

export function anyDirty(): boolean {
  return dirty.size > 0;
}

export function useAnyDirty(): boolean {
  useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => snapshot,
  );
  return dirty.size > 0;
}

/** Register a form's dirty state; cleared automatically on unmount. */
export function useDirty(id: string, isDirty: boolean) {
  useEffect(() => {
    setDirty(id, isDirty);
  }, [id, isDirty]);
  useEffect(() => () => setDirty(id, false), [id]);
}
