## Install

1. Open the DMG, drag **CMDMX** onto **Applications**, then eject the DMG.
2. The app is not notarised, so macOS blocks the first open:
   - **macOS 14:** right-click the app in Applications, choose **Open**, then **Open** again.
   - **macOS 15 or later:** double-click the app once and close the warning. Then open
     **System Settings > Privacy & Security**, scroll down and click **Open Anyway**.
3. After that the app opens normally, until you install the next version.

On first use macOS may ask:
- **Local Network**: allow (needed for Art-Net and sACN).
- **Incoming connections** (firewall): allow (needed for OSC and TouchOSC).

The app starts the engine itself and stops it when you quit, which frees the USB interface for Lightkey.
