"""Synthesize narration per sentence, lay out the timeline, write the audio track.

usage: python3 tts.py <video> ; reads out/<video>.scenes.json, writes
out/<video>.timeline.json and out/<video>.wav
"""
import json, re, sys, os
import numpy as np
import soundfile as sf
from kokoro_onnx import Kokoro

HERE = os.path.dirname(os.path.abspath(__file__))
TTS = os.path.join(HERE, 'build', 'tts')
VOICE = os.environ.get('VOICE', 'am_michael')
SPEED = float(os.environ.get('SPEED', '1.0'))

SAY = [
    (r'\bxSOL\b', 'ex-sol'), (r'\bxStock\b', 'ex-stock'), (r'\bhyUSD\b', 'hy U S D'),
    (r'\bSVI\b', 'S V I'), (r'\bPDA\b', 'P D A'), (r'\bRPC\b', 'R P C'), (r'\bAPI\b', 'A P I'),
    (r'\bSOL\b', 'sol'), (r'\bHylo\b', 'Hylo'), (r'\bPyth\b', 'Pith'), (r'\bdeserialize\b', 'de-serialize'),
    (r'\bSurfnet\b', 'Surf-net'), (r'\bmaths\b', 'maths'),
]

def say(t):
    for a, b in SAY:
        t = re.sub(a, b, t)
    return t

SR = 24000
LEAD, GAP, TAIL = 0.5, 0.38, 0.7

def main(video):
    scenes = json.load(open(os.path.join(HERE, 'build', f'{video}.scenes.json')))
    k = Kokoro(os.path.join(TTS, 'kokoro-v1.0.onnx'), os.path.join(TTS, 'voices-v1.0.bin'))
    audio = []
    t = 0.0
    timeline = []
    for sc in scenes:
        start = t
        sents = []
        cur = LEAD
        pieces = [np.zeros(int(LEAD * SR), dtype=np.float32)]
        for i, s in enumerate(sc['s']):
            samples, sr = k.create(say(s), voice=VOICE, speed=SPEED, lang='en-us')
            assert sr == SR
            d = len(samples) / SR
            sents.append({'t0': cur, 't1': cur + d, 'text': s})
            pieces.append(samples.astype(np.float32))
            cur += d
            gap = GAP if i < len(sc['s']) - 1 else TAIL
            pieces.append(np.zeros(int(gap * SR), dtype=np.float32))
            cur += gap
        seg = np.concatenate(pieces)
        dur = len(seg) / SR
        audio.append(seg)
        timeline.append({'start': start, 'dur': dur, 'sents': sents})
        t += dur
        print(f"{video} scene {len(timeline)} {dur:.1f}s", flush=True)
    full = np.concatenate(audio)
    sf.write(os.path.join(HERE, 'build', f'{video}.wav'), full, SR)
    json.dump(timeline, open(os.path.join(HERE, 'build', f'{video}.timeline.json'), 'w'), indent=1)
    print(video, 'total', round(len(full) / SR, 1), 's')

if __name__ == '__main__':
    main(sys.argv[1])
