/** Reads a dotted path ("mouse.step_px") out of an object; undefined when a part of it is missing. */
export function getIn(root: unknown, path: string): unknown {
  let node: any = root;
  for (const key of path.split('.')) {
    if (node == null) return undefined;
    node = node[key];
  }
  return node;
}

/** A copy of `root` with the value at the dotted path replaced. The objects and arrays on the way are copied, the
 *  rest is shared, so a selector that reads something else still sees the same reference. */
export function setIn<T>(root: T, path: string, value: unknown): T {
  const keys = path.split('.');
  const write = (node: any, depth: number): any => {
    const copy = Array.isArray(node) ? node.slice() : { ...(node ?? {}) };
    const key = keys[depth];
    copy[key] = depth === keys.length - 1 ? value : write(copy[key], depth + 1);
    return copy;
  };
  return write(root, 0);
}
