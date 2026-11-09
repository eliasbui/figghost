import { useEffect } from 'react';
import { isModifierCode, keyLabel, shortcutOf, shortcutOfEvent, shortcutParts } from '@/lib/shortcuts';
import { cx } from '@/lib/text';
import { send, showToast, startRecording, stopRecording, tr, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Keycaps } from './Controls';

/** The field of a shortcut: the keys it holds, or what to do to give it some. A press on it starts capturing. */
export function HotkeyButton({ target, allowClear, label }: { target: string; allowClear?: boolean; label?: string }) {
  const t = useT();
  const shortcut = useApp((s) => shortcutOf(s.state, target));
  const recording = useApp((s) => s.recording?.target === target);
  const spoken = shortcut ? shortcutParts(shortcut).map(keyLabel).join(' ') : t('tiling.none');
  return (
    <button
      type="button"
      className={cx('hotkey gl', recording && 'recording')}
      aria-label={`${label ?? ''} ${spoken}`.trim()}
      onClick={() => startRecording(target, !!allowClear)}
    >
      {recording ? <span className="hint">{t('rec.press')}</span> : shortcut ? <Keycaps shortcut={shortcut} /> : <span className="none">{t('tiling.none')}</span>}
    </button>
  );
}

/** Listens to the keyboard while a shortcut button is capturing, once for the whole window: Escape gives up, Delete
 *  clears where that is allowed, and any other key with a modifier is the new shortcut. */
export function useRecorder(): void {
  const recording = useApp((s) => s.recording);
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (isModifierCode(e.code)) return;
      const bare = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
      if (e.code === 'Escape') {
        stopRecording();
        return;
      }
      if ((e.code === 'Backspace' || e.code === 'Delete') && bare && recording.allowClear) {
        send({ t: 'hotkey', target: recording.target, value: '' });
        stopRecording();
        return;
      }
      const made = shortcutOfEvent(e);
      if ('fault' in made) {
        showToast(tr(made.fault === 'unsupported_key' ? 'fault.unsupported_key' : 'fault.needs_modifier'));
        return;
      }
      send({ t: 'hotkey', target: recording.target, value: made.value });
      stopRecording();
    };
    const onAway = (e: MouseEvent) => {
      if (!(e.target instanceof Element) || !e.target.closest('.hotkey.recording')) stopRecording();
    };
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('mousedown', onAway, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('mousedown', onAway, true);
    };
  }, [recording]);
}
