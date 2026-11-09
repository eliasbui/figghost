import { createContext, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Dialog, Heading, Modal, ModalOverlay } from 'react-aria-components';
import { create } from 'zustand';
import { useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Button } from './Controls';
import { Icon } from './Icon';

// -- a question that has to be answered before something is done

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
  const close = () => useConfirmation.setState({ asked: null });
  return (
    <ModalOverlay
      isOpen={asked !== null}
      onOpenChange={(open) => {
        if (!open) close();
      }}
      isDismissable
      className="scrim"
    >
      <Modal className="dialog glass-pop">
        <Dialog role="alertdialog" className="outline-none">
          <div className="dialog-icon">
            <Icon name="warning" />
          </div>
          <Heading slot="title" level={3} className="dialog-title">
            {asked?.title}
          </Heading>
          <p className="dialog-body">{asked?.body}</p>
          <div className="dialog-actions">
            <Button autoFocus onClick={close}>
              {t('cancel')}
            </Button>
            <Button
              variant="prominent"
              onClick={() => {
                const yes = asked?.onYes;
                close();
                yes?.();
              }}
            >
              {asked?.confirm}
            </Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

// -- a line that comes up from the bottom of the window and goes

const TOAST_MS = 3600;

export function Toast() {
  const toast = useApp((s) => s.toast);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!toast) return;
    setShown(true);
    const timer = setTimeout(() => setShown(false), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast]);
  return (
    <div id="toast" className={shown ? 'show' : undefined} aria-live="polite">
      {toast ? (
        <div className="toast glass-pop" role="status">
          <Icon name="info" weight="fill" />
          <span>{toast.text}</span>
        </div>
      ) : null}
    </div>
  );
}

// -- the groups a page keeps for later

/** True while the page's search reads the text of every page: the groups behind Advanced then count too. */
export const IndexingContext = createContext(false);

/** The groups of a page that few people need. The page shows a button at its foot, and the groups come down in a sheet
 *  over the window. A page uses it once, after everything else. */
export function Advanced({ children }: { children: ReactNode }) {
  const t = useT();
  const page = useApp((s) => s.page);
  const indexing = useContext(IndexingContext);
  const [open, setOpen] = useState(false);
  if (indexing) return <div className="adv">{children}</div>;
  const title = t('adv.title', { page: t(`page.${page}`) });
  return (
    <>
      <div className="page-foot">
        <Button icon="sliders" onClick={() => setOpen(true)}>
          {t('adv.open')}
        </Button>
      </div>
      <ModalOverlay isOpen={open} onOpenChange={setOpen} isDismissable className="scrim sheet-scrim">
        <Modal className="sheet glass-pop">
          <Dialog aria-label={title} className="outline-none">
            <h2 className="sheet-title">{title}</h2>
            <div className="adv">{children}</div>
            <div className="sheet-foot">
              <Button variant="prominent" onClick={() => setOpen(false)}>
                {t('adv.done')}
              </Button>
            </div>
          </Dialog>
        </Modal>
      </ModalOverlay>
    </>
  );
}
