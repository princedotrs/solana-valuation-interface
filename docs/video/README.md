# The videos

| file | length | for |
|---|---|---|
| `svi-pitch.mp4` | 2:45 | the pitch video: why this should exist |
| `svi-technical.mp4` | 5:38 | the technical video: does the mechanism work, and what isn't done |

Both are 1080p, narrated with a synthetic voice (Kokoro TTS, `af_heart`) and
captioned. The narration follows `docs/product/11-video-production.md`, with
changes wherever a line would have claimed something that isn't on screen.

## What is real on screen

- **Code**: the actual repository files, with their real line numbers.
- **Test output, labelled "RUN FOR THIS RECORDING"**: unedited output of
  `cargo test` (svi-core `--lib`: 7, svi-stock-adapter: 26, svi-keeper: 31) and
  `node --test site/test/*.mjs` (15), run on 25 Sep 2026.
- **Terminal labelled "REPLAY"**: the xSOL surfnet run, rebuilt line by line
  from `docs/validation/2026-09-18-xsol-nav-surfnet.md`. It is not a screen
  capture. The recording environment had no Solana RPC access, so the on-chain
  run could not be repeated live.
- **The staleness figures** (6–34 days, 78 days, 2 of 3 missing, 401) are the
  18 Sep measurements, labelled with that date. They were not re-measured.

To replace either video with a human-recorded one, use the shot list in
`docs/product/11-video-production.md`.

## Rebuild

From `docs/video/src/` (you need Node, Playwright + Chromium, Python with
`kokoro-onnx soundfile numpy`, and ffmpeg):

```bash
mkdir -p build/logs build/shots build/tts
# the Kokoro model files from github.com/thewh1teagle/kokoro-onnx releases -> build/tts/
# the test logs the scenes quote -> build/logs/{svi-core-lib,svi-stock-adapter,svi-keeper,site}.log
(cd ../../.. && python3 -m http.server 8099 &) ; node shot.js        # site screenshots
node -e "const s=require('./scenes.js');for(const k of ['pitch','tech'])require('fs').writeFileSync('build/'+k+'.scenes.json',JSON.stringify(s[k]))"
VOICE=af_heart python3 tts.py pitch && VOICE=af_heart python3 tts.py tech
node render.js pitch && node render.js tech                          # build/<video>.mp4
```
