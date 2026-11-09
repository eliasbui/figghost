import { create } from 'zustand';

// What the lists of Mole share: the rows chosen in each (they are removed together), and the row the chart of the analysis
// points at.
export const useMoleUi = create<{ chosen: Record<string, string[]>; lit: string | null }>()(() => ({ chosen: {}, lit: null }));

export function setChosen(kind: string, paths: string[]): void {
  useMoleUi.setState((s) => ({ chosen: { ...s.chosen, [kind]: paths } }));
}

export function setLit(path: string | null): void {
  useMoleUi.setState({ lit: path });
}
