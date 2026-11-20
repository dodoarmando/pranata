import { useRouter } from '@tanstack/react-router';
import { useCallback, useEffect, useRef, useState } from 'react';

// How long the writer pauses before a Draft saves itself.
const AUTOSAVE_MS = 1500;

/**
 * A website item's fields as its editor holds them. A Draft saves itself once the writer
 * pauses, one save at a time so none lands out of order, and on leaving the page; a Published
 * item waits for `flush`. `save` answers the API's status, and `onResult` hears every one.
 */
export function useAutosave<T>(
  initial: () => T,
  published: boolean,
  save: (fields: T) => Promise<number>,
  onResult: (status: number) => void,
) {
  const router = useRouter();
  const [fields, setFields] = useState(initial);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [savedAt, setSavedAt] = useState<Date | null>(null);
  // Bumped on every edit, so a save only clears `dirty` when nothing changed meanwhile.
  const edits = useRef(0);
  const latest = useRef({ fields, dirty, save, onResult });
  latest.current = { fields, dirty, save, onResult };

  const change = (patch: Partial<T>) => {
    edits.current += 1;
    setFields((current) => ({ ...current, ...patch }));
    setDirty(true);
  };

  /** Saves the fields now; whether it worked. */
  const flush = useCallback(async () => {
    const edit = edits.current;
    setSaving(true);
    const result = await latest.current.save(latest.current.fields).finally(() => setSaving(false));
    latest.current.onResult(result);
    if (result >= 300) return false;
    if (edits.current === edit) setDirty(false);
    setSavedAt(new Date());
    await router.invalidate();
    return true;
  }, [router]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: every edit restarts the pause.
  useEffect(() => {
    if (!dirty || saving || published) return;
    const timer = setTimeout(flush, AUTOSAVE_MS);
    return () => clearTimeout(timer);
  }, [dirty, saving, fields, published, flush]);

  // Leaving within the pause still saves a Draft; the browser warns before unsaved changes go.
  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (latest.current.dirty) event.preventDefault();
    };
    window.addEventListener('beforeunload', warn);
    return () => {
      window.removeEventListener('beforeunload', warn);
      if (latest.current.dirty && !published) void latest.current.save(latest.current.fields);
    };
  }, [published]);

  /** Forgets unsaved changes, as when the item is deleted. */
  const discard = () => {
    setDirty(false);
    latest.current.dirty = false;
  };

  return { fields, change, dirty, saving, savedAt, flush, discard };
}
