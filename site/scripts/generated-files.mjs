// Cleanup is limited to generated directories below the caller's explicit root.
import {existsSync, lstatSync, realpathSync, rmSync} from 'node:fs';
import {isAbsolute, relative, resolve, sep} from 'node:path';

export function removeGeneratedDirectory(directory, root) {
  const target = resolve(directory), boundary = resolve(root);
  const inside = (base, path) => {
    const rel = relative(base, path);
    return rel !== '' && !isAbsolute(rel) && rel !== '..' && !rel.startsWith(`..${sep}`);
  };
  if (!inside(boundary, target)) throw new Error(`Generated cleanup escaped its root: ${target}`);
  if (!existsSync(target)) return;
  if (lstatSync(target).isSymbolicLink() || !inside(realpathSync(boundary), realpathSync(target))) {
    throw new Error(`Generated cleanup must stay inside its real root: ${target}`);
  }
  rmSync(target, {recursive: true, force: true});
}
