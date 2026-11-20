// A question that has to be answered before something is done, a line that comes up and goes, and the groups a page keeps for
// later. All of them are Astryx's.
import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { AlertDialog } from '@astryxdesign/core/AlertDialog';
import { Dialog, DialogHeader } from '@astryxdesign/core/Dialog';
import { useToast } from '@astryxdesign/core/Toast';
import { VStack } from '@astryxdesign/core/VStack';
import { create } from 'zustand';
import { useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Button } from './controls';
import { IndexingContext } from './layout';
import { useContext } from 'react';

export { IndexingContext };

interface Confirmation {
  title: string;
  body: string;
  confirm: string;
  onYes: () => void;
}

const useConfirmation = create<{ asked: Confirmation | null }>()(() => ({ asked: null }));

/** Asks, in a dialog over the window, whether to go on; `onYes` runs only if the answer is yes. */
export function confirmDialog(text: { title: string; body: string; confirm: string }, onYes: () => void): void {
  useConfirmation.setState({ asked: { ...text, onYes } });
}

export function ConfirmHost() {
  const t = useT();
  const asked = useConfirmation((s) => s.asked);
  // What was asked stays on the dialog while it closes.
  const [last, setLast] = useState<Confirmation | null>(null);
  useEffect(() => {
    if (asked) setLast(asked);
  }, [asked]);
  const shown = asked ?? last;
  const close = () => useConfirmation.setState({ asked: null });
  return (
    <AlertDialog
      isOpen={asked !== null}
      onOpenChange={(open) => {
        if (!open) close();
      }}
      title={shown?.title ?? ''}
      description={shown?.body ?? ''}
      cancelLabel={t('cancel')}
      actionLabel={shown?.confirm ?? ''}
      onAction={() => {
        const yes = asked?.onYes;
        close();
        yes?.();
      }}
    />
  );
}

/** What the app says in a line, for a few seconds. */
export function Toast() {
  const toast = useApp((s) => s.toast);
  const show = useToast();
  useEffect(() => {
    if (toast) show({ body: toast.text, uniqueID: String(toast.id), autoHideDuration: 3600 });
    // `show` is made again on every render; the toast is what matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toast]);
  return null;
}

/** The groups of a page that few people need. The page shows a button at its foot, and the groups come up in a dialog over
 *  the window. A page uses it once, after everything else. */
export function Advanced({ children }: { children: ReactNode }) {
  const t = useT();
  const page = useApp((s) => s.page);
  const indexing = useContext(IndexingContext);
  const [open, setOpen] = useState(false);
  if (indexing) return <>{children}</>;
  const title = t('adv.title', { page: t(`page.${page}`) });
  return (
    <>
      <div className="page-foot">
        <Button icon="sliders" onClick={() => setOpen(true)}>
          {t('adv.open')}
        </Button>
      </div>
      <Dialog isOpen={open} onOpenChange={setOpen} width={560}>
        <DialogHeader title={title} onOpenChange={setOpen} />
        <VStack gap={6} padding={4}>
          {children}
          <Button variant="primary" onClick={() => setOpen(false)}>
            {t('adv.done')}
          </Button>
        </VStack>
      </Dialog>
    </>
  );
}
