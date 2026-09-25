// node render.js <video> [--preview]   -> out/<video>.mp4 (or preview PNGs)
const { chromium } = require('playwright');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const video = process.argv[2];
const preview = process.argv.includes('--preview');
const FPS = 24;
const OUT = path.join(__dirname, 'build');
const scenes = JSON.parse(fs.readFileSync(path.join(OUT, `${video}.scenes.json`)));
const tl = JSON.parse(fs.readFileSync(path.join(OUT, `${video}.timeline.json`)));
const total = tl.reduce((a, s) => a + s.dur, 0);
const NAME = { pitch: 'pitch · 3 min', tech: 'technical walkthrough' }[video];

(async () => {
  const browser = await chromium.launch({ args: ['--allow-file-access-from-files'] });
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  await page.goto('file://' + path.join(__dirname, 'stage.html'));
  await page.evaluate(([n, t]) => window.setup(n, t), [NAME, total]);

  let ff = null;
  if (!preview) {
    ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'mjpeg', '-i', '-',
      '-i', path.join(OUT, `${video}.wav`),
      '-c:v', 'libx264', '-preset', 'medium', '-crf', '19', '-pix_fmt', 'yuv420p', '-r', String(FPS),
      '-c:a', 'aac', '-b:a', '160k', '-shortest', '-movflags', '+faststart', path.join(OUT, `${video}.mp4`)], { stdio: ['pipe', 'inherit', 'inherit'] });
  }
  const write = (buf) => new Promise((res) => (ff.stdin.write(buf) ? res() : ff.stdin.once('drain', res)));

  let frameNo = 0;
  for (let i = 0; i < scenes.length; i++) {
    const tm = tl[i];
    await page.evaluate(([s, t, st]) => window.load(s, t, st), [scenes[i], tm, tm.start]);
    const f0 = Math.round(tm.start * FPS), f1 = Math.round((tm.start + tm.dur) * FPS);
    if (preview) {
      for (const frac of [0.5, 0.97]) {
        await page.evaluate((t) => window.frame(t), tm.dur * frac);
        await page.screenshot({ path: path.join(OUT, `prev_${video}_${String(i).padStart(2, '0')}_${frac}.png`) });
      }
      continue;
    }
    for (let f = f0; f < f1; f++) {
      await page.evaluate((t) => window.frame(t), f / FPS - tm.start);
      const buf = await page.screenshot({ type: 'jpeg', quality: 90 });
      await write(buf);
      frameNo++;
    }
    console.log(`${video}: scene ${i + 1}/${scenes.length} done (${frameNo} frames)`);
  }
  await browser.close();
  if (ff) { ff.stdin.end(); await new Promise((r) => ff.on('close', r)); }
  console.log(video, 'done', total.toFixed(1), 's');
})();
