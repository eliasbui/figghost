# The character of Jarvis

`crates/app-ui/assets/jarvis/jarvis.png` holds twelve square frames of 160 pixels in one row, in the order of
`macos_sys::jarvis_ui::frame`: idle, blink, look left, look right, listen, talk open, talk half, happy, think, error,
wave a, wave b.

They were made in three steps:

1. `agy -p "$(cat prompt.txt)" --dangerously-skip-permissions` in an empty folder: agy's image generator drew the 4 x 3
   sheet (`sheet.png` is that sheet, made smaller).
2. `python3 -I key.py jarvis-sheet.png keyed.png out` turns the black around the sprites into the magenta key that
   `docs/mascots/build.py` reads.
3. `python3 -I cut.py keyed.png frames.png 250` cuts each sprite around its head into a square of 250 pixels, then
   `sips -z 160 1920 frames.png --out jarvis.png` makes them 160.

To draw another character, change the prompt and repeat; keep the order of the frames.
