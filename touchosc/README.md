# TouchOSC layout

- `ColourmotionDMX.touchosc`: TouchOSC Mk1 layout (iPad, landscape) for the console's built-in OSC schema.
- `addresses.txt`: what the layout sends and shows; the Rust test `ctl_64` checks it against the schema.
- `generate.py`: writes both. Checked in; CI runs `python3 touchosc/generate.py --check`.
- `serve.sh`: serves the layout to the iPad.

Regenerate with `python3 touchosc/generate.py --faders N`, where N is the show's `faders_per_page` (default 8).

## Load it
1. `touchosc/serve.sh`, then on the iPad TouchOSC -> Layout -> Add -> pick the Mac under Found Hosts
   (or type the Mac's IP under Host, then Download). Ctrl-C stops it.
2. Fallback: open the file in the TouchOSC Editor and press Sync.

## Connect
TouchOSC -> Connections -> OSC: Host = the Mac's IP, Port (outgoing) 8000, Port (incoming) 9000.
Enable OSC in the console's Control screen.
