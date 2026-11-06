# The phone remote

Alone on the stage with the console at the back, you can walk the patch from your phone: CMDMX serves
a page on the network, behind a login you set in the console. The phone sees the patch and may change
a fixture's universe, address and name, and highlight it to see which lamp it is. Adding, removing and
renumbering fixtures stay on the console.

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

## Walking the patch from a phone

1. Turn the remote on in the **Remote** screen and scan its QR code with the phone's camera; the phone must be on
   the same network as the Mac. The page needs no internet, so it also works on a church Wi-Fi without it.
2. Log in with the username and password from the Remote screen. The phone remembers the login: a reload, or the
   phone waking up, opens the patch again without asking. After the engine restarts it asks once more.
3. The list shows every fixture in number order, such as `201 · Front L · Phantom 25 · 1.37–1.50`: number, name,
   model and the channels it takes. Type a number in the search field and the list starts at it (Go opens it), or part of a
   name to show only those fixtures. Tap a fixture to open its card.
4. Turn **Highlight** on in the card. The fixture becomes the console's selection and shows its highlight values on
   the rig, so the lamp that lights is the one on the card; the operator at the console sees the selection change.
   **Next** and **Previous** take the highlight to the neighbour in number order: one tap per lamp. A lamp that
   lights where you did not expect it, or one that stays dark, has a wrong address.
5. Put it right on the card: type the universe and address with the number keypad and tap **Save**. The card shows
   the new channels, or the console's refusal in the card itself, such as `201 would overlap 301 at 1.45`. Type a
   name in the name field; it is saved when you leave the field. Each change is one undo step at the console.
   When the warning names a fixture in the way, **Park 301 and move** parks it and moves this one.
6. A lamp that does not hang: tap **Park** on its card. It stays in the list, greyed as *parked (was 1.37)*,
   keeps its groups and cues and frees its channels; Next skips it while Highlight is on. **Unpark** puts it back at
   its old address; when that address is taken by now the card shows the clash and the address fields to give it
   a new one.
7. **Back** to the list, or turning Highlight off, switches the highlight off again; the selection stays.

When the connection drops the page says **Console offline** and tries again every 2 seconds, so walk back into
range or wait for the engine to come back. While the console is locked the page says **Console locked** and its
fields, Save and Highlight are greyed; Previous and Next still move the card. When another show is opened at the console, the
page shows its list and its name at the top.

Remember that the password crosses the network readable (see *Plain HTTP* above).
