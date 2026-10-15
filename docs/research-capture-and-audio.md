# How CleanShot X, SoundSource and Boom 3D do it

Notes from a static look at the shipped apps, made before building the capture and per-app audio
features. Only public facts were used: the frameworks and system functions each binary imports, the
Objective-C class names it declares, and its property lists. No vendor code or resource was copied;
figghost re-implements the features with public macOS APIs.

Builds examined (official downloads, Developer ID signed and notarized):

| App | Version | Notes |
|---|---|---|
| CleanShot X | 5.0.1 | SHA-256 matched the Homebrew cask |
| SoundSource | 6.1.5 | |
| Boom 3D | 2.3.2 | |

Tools: the `rea` command line (inspect-macho, inspect-signature, inspect-plist, inspect-artifact,
project-apple-application-graph, inspect-native-dispatch-metadata).

## CleanShot X

- Capture: ScreenCaptureKit (`SCStream`, `SCContentFilter`) for the live screen and windows, with
  `CGDisplayCreateImage` and `CGWindowListCreateImage` as older paths. A shielding-level window
  (`CGShieldingWindowLevel`) shows the frozen screen while an area is picked.
- Recording: frames go to `AVAssetWriter`. GIF and WebP exports use bundled `gifsicle` and `cwebp`.
- Text and codes: Vision `VNRecognizeTextRequest` and `VNDetectBarcodesRequest`.
- Scrolling capture: synthetic scroll events (`CGEventCreateScrollWheelEvent2`, `CGEventPost`) and
  `VNTranslationalImageRegistrationRequest` to find how far each frame moved.
- Editor: Core Image filters for blur and pixelate. Global input through `CGEventTapCreate`.

## SoundSource

- Per-app audio through Core Audio process taps (macOS 14.2 and later): `AudioHardwareCreateProcessTap`
  with a `CATapDescription`, a private aggregate device holding the tap, and an IOProc that processes
  the tapped audio and plays it to the chosen output. Helper binaries `streamkeeper` and
  `arkinstaller` carry the same symbols. Class names include `ACPCATapNode` and `ACPSource`.
- Equalizer: `vDSP_biquad` filter chains.
- Process policy: `SSAudioPolicy.plist` groups helper processes under the app they belong to (browser
  renderers, for example) and lists processes that are never captured.
- Volume keys for outputs without hardware volume: a `CGEventTapCreate` tap ("Super Volume Keys").

## Boom 3D

- Also uses process taps, plus `AudioComponentRegister` (an in-process audio unit), `AVAudioEngine`,
  and a legacy HAL plug-in, `BoomAudio.driver`, for older systems.

## What figghost took from this

- Process taps, not a HAL driver: no installer, no kernel or driver code, and the symbols are looked
  up at run time so the app still starts on systems without them.
- One engine for the SoundSource and Boom 3D features: per-app volume to 400 % behind a limiter, mute,
  output routing, a 10-band equalizer with presets, Magic Boost and Night mode as compressors, and
  stereo width instead of HRTF surround.
- Only apps whose settings differ from the defaults are tapped; helper processes are grouped under
  their app through the responsible process ID.
- Capture with ScreenCaptureKit only; recordings through `SCRecordingOutput` (macOS 15 and later)
  instead of a hand-driven `AVAssetWriter`; GIFs with ImageIO instead of a bundled encoder.
- Left out: CleanShot Cloud, Boom radio, hosting third-party audio units, HRTF surround, pitch shift,
  zoom on click while recording, and a headphone equalizer profile library.
