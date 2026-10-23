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

## Serial ports

A device can be reached on a serial port of this machine (RS-232, RS-422, a
USB serial adapter): a serial-only device, or one whose spec offers both
links, chosen with the `link` setting (`"tcp"` or `"serial"`). The
`serial_port` setting names the port, and `core.serial_ports()` lists this
machine's ports to choose from (names only; empty on Android and iOS):

```python
port = core.serial_ports()[0]  # {"name": "COM3"}
device = core.open({"device": "extron-matrix", "model": "mmx-42-62",
                    "settings": {"link": "serial", "serial_port": port["name"]}})  # no host needed
```


## MIDI ports

A MIDI device can be reached on a MIDI port of this machine (a USB or other
class-compliant MIDI interface or instrument, or another program's port):
`generic-midi` and `midi-show-control` take the `link` setting `"port"` and
name the ports in `midi_input` and `midi_output`. `core.midi_ports()` lists
this machine's ports, each with whether it is an input, an output or both
(empty in a build without a MIDI integration and on Android):

```python
keys = next(p for p in core.midi_ports() if p["input"] and p["output"])
device = core.open({"device": "generic-midi", "model": "generic",
                    "settings": {"link": "port", "midi_input": keys["name"],
                                 "midi_output": keys["name"]}})
```
## Files sent to devices

A command's `binary` parameter (a show file, a media file, an image) takes
the file's bytes as base64 text or a file on this machine as
`{"file": "<path>"}`:

```python
core.execute(device, "load_show_file", {"file": {"file": "D:/Shows/Main.watch"}})
```

The core reads the file from disk as the request goes out, never all at once,
so its size does not matter; there is no need to read it into `bytes`. A file
that is missing or cannot be read raises `IntegrationsError` `invalid_params`
naming the parameter (SPEC.md §4, "Binary request bodies").

## Tests

The tests drive devices of several integrations, so they need an extension
built with every integration (the default).
