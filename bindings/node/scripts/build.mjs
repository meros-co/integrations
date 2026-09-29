// Build the addon with cargo and place it where index.js looks for it:
// meros-integrations.<platform>-<arch>.node, so prebuilt binaries for several
// platforms can ship in one package.
import { execFileSync } from 'node:child_process';
import { copyFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..');
const debug = process.argv.includes('--debug');
const profile = debug ? 'debug' : 'release';

execFileSync('cargo', ['build', '-p', 'meros-integrations-node', ...(debug ? [] : ['--release'])], {
  cwd: root,
  stdio: 'inherit',
});

const library = {
  win32: 'meros_integrations_node.dll',
  darwin: 'libmeros_integrations_node.dylib',
  linux: 'libmeros_integrations_node.so',
}[process.platform];
if (!library) throw new Error(`unsupported platform ${process.platform}`);

const target = join(here, '..', `meros-integrations.${process.platform}-${process.arch}.node`);
copyFileSync(join(root, 'target', profile, library), target);
console.log(`built ${target}`);
