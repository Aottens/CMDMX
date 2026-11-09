# Research: LG Lighting Smart 5, a low-priced competitor

Status: research only, no acceptance criteria. Written 2026-11-09, after the owner pointed at an Alibaba listing.
Each point below is a candidate; check it against the Titan manual before it becomes a criterion.

How it was found: the maker's product page, its 50-page English user manual (read in full), and marketplace pages.
Nobody has touched the console. The Alibaba listing itself sat behind a CAPTCHA and was not read, and no independent
review was found. The manual warns that its illustrations show an earlier software edition, so a feature it does not
mention may still exist.

Sources:
- [Product page](https://www.lglighting.net/lglighting-products/smart-5)
- [User manual (PDF)](https://d0756492-e326-4fa7-8698-250cfc4e0635.usrfiles.com/ugd/53766f_8add9199c06043fe9ba454d2172ec306.pdf)
- [Alibaba listing, not read](https://www.alibaba.com/product-detail/SMART5-Lighting-Console-Large-Scale-Stage_1601436741020.html)
- [Accio, which quotes the listing](https://www.accio.ai/find-product/smart-3-lighting-console)

## What it is
A full-size desk from LG Lighting in Foshan (not the Korean LG), a maker of moving heads and lasers. Sold by the trader
Kuangwu (Foshan) Audio Equipment at about $910–950, minimum order one, according to Accio. Shipping, duty and VAT are
unknown. The smaller Smart 3 (2048 channels, 4 DMX ports) is about $700–720. Sold for stages, weddings, bars and DJ rental.

| Item | Maker's specification |
|---|---|
| Channels | 4096 (8 universes), expandable to 32768 (64 universes) |
| Outputs | 8 optically isolated DMX ports, 1 Art-Net port, Wi-Fi |
| Surface | 18 playback faders × 60 pages, 8 preset faders, 4 encoders, 44 playbacks at once |
| Computer | Linux, 10th-generation Intel, 15.6" multi-touch |
| Fixture formats | R20, D4, XML, an editor on the console |
| Time and audio | Internal player, MIDI and LTC timecode; MP3, WAV, FLAC, APE |
| Remote | App for Android and iOS, Android wall panel |

The sources disagree. The manual's older text says 12 faders per page, 10.4", 4 DMX ports and expansion to 8 outputs
"by connecting the network extender". The product size is 680×570×190 mm on the site and 760×665×335 mm in the manual.
How the 64 outputs are reached is not stated.

## How it operates
Titan's working model on its own screens, with some MA words. Whether any code or data is shared cannot be told.
- As Titan and CMDMX: attribute groups I P C G B E S FX, *Wait For Go* / *After Previous*, Global and Normal palettes,
  fixture overlap, Unfold, four fader modes, the R20 and D4 libraries.
- As MA: *Pools* and *Executor Editor*.
- Own words: a cue is a "scene", a cue list a "chase", a palette a "material".
- The firmware menu names F1, F2 and F3 boards, and another seller offers an "F2 F3" controller: possibly one platform
  sold under several names. A hint, not proof.

## Worth learning from
1. **Timecode on the song:** record cues live against an audio file, adjust start and end on a waveform, put the shows
   in a list with four play modes. MIDI and LTC use the same recorder (IDEA-05).
2. **Output routing:** any line goes to several outputs at once, including Art-Net and the Depence 2 visualiser.
3. **Hand-drawn legends** on groups, palettes and scenes.
4. **Locking a fader** across page changes, and 8 faders that never page.
5. **A second selection state:** a tap turns a selected fixture green, kept in the selection but not edited.

## Where CMDMX is ahead, as far as the manual shows
- No undo: deleting a step is called irreversible, replacing cannot be restored, and deleting a fixture deletes its
  programming. CMDMX has Undo, Park and Fixture Exchange.
- Tracking is never mentioned.
- sACN, RDM and GDTF are not in the manual. Resellers claim sACN and RDM; the screens show Art-Net broadcast only.
- Autosave intervals look like 15 to 60 minutes. Fixed master passwords for the lock screen are printed in the public
  manual.
- No offline editor is described; a show moves only to a console of the same model.

## What it means
- As software it is no direct competitor: it is a closed appliance, CMDMX an open program on a Mac.
- For the hardware desk (IDEA-23) it is: 26 faders in a finished desk for about $950 undercuts the ~€1000–2500 build.
  The case for the desk cannot rest on price. It can rest on openness, repair, show-proof behaviour and long-term
  updates that do not depend on one maker.

## Open
- The real delivered price, the warranty (one year, from a poor third-party copy of the Smart 3 manual) and how often
  the software is updated.
- A hands-on comparison, if the owner ever gets near one.
