# The phone remote

Alone on the stage with the console at the back, you can walk the patch from your phone: CMDMX serves
a page on the network, behind a login you set in the console. The phone sees the patch and may change
a fixture's universe, address and name, and highlight it to see which lamp it is. Adding, removing and
renumbering fixtures stay on the console.

The page itself arrives in a later update. Until then the address answers with a placeholder line,
which is enough to check that your phone reaches the Mac.

## The Remote screen

Click **Remote** in the rail to open it.

- **Username** and **Password**: the login for the phone, 1 to 64 characters each. The password shows
  as dots and is never shown again, so type it each time you turn the remote on.
- **Port**: 7320 unless you choose another (1024 to 65535).
- **Turn on** starts the remote with this login, and applies a changed login while it runs. It is
  refused with an empty field. **Turn off** stops it and closes every phone.

While the remote is on, the screen says how many phones are connected and shows the address for each
network the Mac is on, such as `http://192.168.1.20:7320`, with a QR code of the first. Scan the code
with the phone's camera to open the page.

The login, the port and whether the remote is on are kept by the engine, not in the show: opening
another show keeps them, and after a restart the remote is on again. A phone that logged in stays
logged in until the engine restarts or you set a new login. After five wrong passwords from one phone,
it has to wait 30 seconds.

The remote works while the console is locked, but a locked console refuses every change from the phone.

**Plain HTTP:** the page is not encrypted, so the password crosses the network readable to anyone on it
who listens. The login keeps strangers out of the patch, not out of the Wi-Fi. Use a password you use
nowhere else.

If the port is already taken by another program, the message line says `Remote: port 7320 is in use`
and the remote stays off: choose another port.
