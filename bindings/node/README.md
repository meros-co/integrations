# Node package

`@meros/integrations`: the integrations core as a Node addon. `index.js` and
`index.d.ts` are the interface; the addon is built from this directory's
crate.

```
npm run build          # release build, every integration
npm run build:debug    # debug build
npm test
```

## Choosing integrations

The addon contains every integration unless you name the ones you want when
you build it. Name spec ids, vendor groups (`vendor-sennheiser`) or `all`,
separated by commas, with `--integrations=` or the `MEROS_INTEGRATIONS`
environment variable:

```
npm run build -- --integrations=sennheiser-ew-dx,shure-wireless
MEROS_INTEGRATIONS=sennheiser-ew-dx,shure-wireless npm run build
MEROS_INTEGRATIONS=vendor-sennheiser,vendor-shure npm run build
```

Each name is a feature of the core (the list is in `crates/core/Cargo.toml`),
and the build fails for a name that is not one. Integrations left out have
neither their spec, their code nor their dependencies in the addon. The
script runs `cargo build -p meros-integrations-node --no-default-features
--features meros-integrations/<name>,...`, which can also be run directly.

At run time a core can be narrowed further to some of the integrations the
addon was built with:

```js
const core = new Core({ devices: ['sennheiser-ew-g3-g4', 'shure-wireless'] });
```

`devices` takes spec ids, vendor groups or `'all'`. The catalogue then lists
only those devices, opening any other throws `not_selected`, and discovery
runs only the protocols that find them. Opening a device whose integration
was not built in throws `not_built`, naming the feature to build with.

## Files sent to devices

A command's `binary` parameter (a show file, a media file, an image) takes
the file's bytes as base64 text or a file on this machine as
`{ file: '<path>' }`:

```js
await core.execute(device, 'load_show_file', { file: { file: 'D:/Shows/Main.watch' } });
```

The core reads the file from disk as the request goes out, never all at once,
so its size does not matter; there is no need to read it into a `Buffer`. A
file that is missing or cannot be read rejects with an `IntegrationsError`
`invalid_params` naming the parameter (SPEC.md §4, "Binary request bodies").

## Tests

`npm test` drives devices of several integrations, so it needs an addon built
with every integration (the default).
