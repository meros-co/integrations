// Build the addon with cargo and place it where index.js looks for it:
// meros-integrations.<platform>-<arch>.node, so prebuilt binaries for several
// platforms can ship in one package.
//
// Which integrations the addon contains is chosen here, as a comma-separated
// list of spec ids, vendor groups (vendor-sennheiser) or `all`:
//
//   npm run build -- --integrations=sennheiser-ew-dx,shure-wireless
//   MEROS_INTEGRATIONS=sennheiser-ew-dx,shure-wireless npm run build
//
// Without a list the addon contains every integration.
import { execFileSync } from 'node:child_process';
import { copyFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..');
const debug = process.argv.includes('--debug');
const profile = debug ? 'debug' : 'release';

const flag = process.argv.find((a) => a.startsWith('--integrations='));
const list = flag ? flag.slice('--integrations='.length) : process.env.MEROS_INTEGRATIONS ?? '';
const integrations = list.split(/[\s,]+/).filter(Boolean);
if (integrations.some((name) => !/^[a-z0-9-]+$/.test(name))) {
  throw new Error(`integration names are spec ids, vendor groups or 'all': ${list}`);
}
// Each name is a feature of the core (crates/core/Cargo.toml); cargo refuses
// one it does not have.
const features = integrations.length
  ? ['--no-default-features', '--features', integrations.map((name) => `meros-integrations/${name}`).join(',')]
  : [];

execFileSync('cargo', ['build', '-p', 'meros-integrations-node', ...(debug ? [] : ['--release']), ...features], {
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
console.log(`built ${target} with ${integrations.length ? integrations.join(', ') : 'every integration'}`);
