import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from '@/app/App';
import '@/styles/index.css';

// While the page is being built, `window.__go('notch')` opens a page, for looking at each one.
if (import.meta.env.DEV) void import('@/stores/app').then(({ go }) => Object.assign(window, { __go: go }));

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
