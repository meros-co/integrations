# Python package

`meros-integrations`: the integrations core as a Python extension
(`meros_integrations`). Release wheels are built with maturin; `build_dev.py`
builds the extension in place for development and tests.

```
python build_dev.py                          # every integration
python -m unittest discover -s tests
```

## Choosing integrations

The extension contains every integration unless you name the ones you want
when you build it. Name spec ids, vendor groups (`vendor-sennheiser`) or
`all`; each is a feature of the core (the list is in
`crates/core/Cargo.toml`), and the build fails for a name that is not one.
Integrations left out have neither their spec, their code nor their
dependencies in the extension.

For development, with `--integrations` or the `MEROS_INTEGRATIONS`
environment variable:

```
python build_dev.py --integrations sennheiser-ew-dx,shure-wireless
MEROS_INTEGRATIONS=vendor-sennheiser,vendor-shure python build_dev.py
```

For a wheel, pass the same names to maturin as features of the core:

```
maturin build --release --no-default-features \
  --features meros-integrations/sennheiser-ew-dx,meros-integrations/shure-wireless
```

or, when pip builds the wheel from source, through maturin's
`MATURIN_PEP517_ARGS`:

```
MATURIN_PEP517_ARGS="--no-default-features --features meros-integrations/sennheiser-ew-dx,meros-integrations/shure-wireless" pip wheel .
```

At run time a core can be narrowed further to some of the integrations the
extension was built with:

```python
core = Core(devices=["sennheiser-ew-g3-g4", "shure-wireless"])
```

`devices` takes spec ids, vendor groups or `"all"`. The catalogue then lists
only those devices, opening any other raises `IntegrationsError`
`not_selected`, and discovery runs only the protocols that find them. Opening
a device whose integration was not built in raises `not_built`, naming the
feature to build with.

The tests drive devices of several integrations, so they need an extension
built with every integration (the default).
