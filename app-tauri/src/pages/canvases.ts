import type { ComponentType } from 'react';

// A feature can also draw a window of its own: the same page told which canvas to show (`?canvas=<id>`). The canvas of a
// feature is the default export of pages/<id>/Canvas.tsx.
const modules = import.meta.glob<{ default: ComponentType }>('./*/Canvas.tsx');

export function canvasLoader(id: string): (() => Promise<{ default: ComponentType }>) | undefined {
  return modules[`./${id.replace(/[^a-z]/g, '')}/Canvas.tsx`];
}
