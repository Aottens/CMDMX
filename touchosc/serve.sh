#!/bin/sh
# CTL-65: serve the layout to TouchOSC Mk1 like the TouchOSC Editor's Sync does.
# On the iPad: TouchOSC -> Layout -> Add -> the Mac under "Found Hosts" (or type the Mac's IP
# under Host and tap Download). Stop with Ctrl-C.
#
# Sources: Hexler's Mk1 manual says the editor "starts a server process" and the app lists
# computers found by Bonjour under Found Hosts, with manual host entry as fallback
#   https://hexler.net/touchosc-mk1/manual/editor-sync
#   https://hexler.net/touchosc-mk1/manual/configuration-layout-transfer-wifi
# Proven on the church iPad mini (Mk1 1.9.8, 2026-10-25): the app sends a plain HTTP GET / to
# port 9658 and wants the bare index.xml, not the .touchosc zip (the zip downloads but the app
# silently keeps the old layout). Bonjour type _touchoscedit._tcp (the older
# _touchosceditor._tcp is announced too). If the iPad does not list the Mac, enter its IP
# manually; if the download fails, use the TouchOSC Editor's Sync.
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
PORT=9658
dns-sd -R "CMDMX" _touchoscedit._tcp local $PORT >/dev/null &
A=$!
dns-sd -R "CMDMX" _touchosceditor._tcp local $PORT >/dev/null &
B=$!
trap 'kill $A $B 2>/dev/null' EXIT INT TERM
echo "Serving CMDMX.touchosc on port $PORT (Ctrl-C to stop)"
python3 -u - "$DIR/CMDMX.touchosc" $PORT <<'PY'
import sys
import zipfile
from http.server import BaseHTTPRequestHandler, HTTPServer

data = zipfile.ZipFile(sys.argv[1]).read("index.xml")


class H(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Disposition", 'attachment; filename="CMDMX.touchosc"')
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, fmt, *args):
        print("request from", self.client_address[0], self.path)


try:
    HTTPServer(("", int(sys.argv[2])), H).serve_forever()
except KeyboardInterrupt:
    pass
PY
