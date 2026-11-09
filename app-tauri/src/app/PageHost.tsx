import { lazy, Suspense, useLayoutEffect, useRef } from 'react';
import type { ComponentType, LazyExoticComponent } from 'react';
import { PageHero } from '@/components/macos/Page';
import { hasOwnTop, pageLoader } from '@/pages/registry';
import { useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { PageBoundary } from './PageBoundary';

// The code of a page is fetched when the page is first opened.
const components = new Map<string, LazyExoticComponent<ComponentType>>();

function componentOf(id: string): LazyExoticComponent<ComponentType> {
  let component = components.get(id);
  if (!component) {
    component = lazy(pageLoader(id));
    components.set(id, component);
  }
  return component;
}

/** The page that is open, under the top it has. */
export function PageHost() {
  const id = useApp((s) => s.page);
  const t = useT();
  const scroller = useRef<HTMLDivElement>(null);
  const Page = componentOf(id);
  useLayoutEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0;
  }, [id]);
  return (
    <div id="scroller" ref={scroller}>
      <div className="page" key={id}>
        {hasOwnTop(id) ? null : <PageHero id={id} />}
        <PageBoundary page={id} message={t('fault.page_failed')}>
          <Suspense fallback={null}>
            <Page />
          </Suspense>
        </PageBoundary>
      </div>
    </div>
  );
}
