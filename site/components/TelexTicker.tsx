'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { encode, type Column } from '@/lib/ita2';
import s from './TelexTicker.module.css';

const COL = 12; // tape advances one column per character
const ROW = 8;
const TOP = 10;
const TAPE_H = TOP * 2 + ROW * 5; // five channels plus the feed row between channels 2 and 3
const TEXT_Y = TAPE_H + 16;
const HEIGHT = TAPE_H + 26;
const CHANNEL_ROWS = [0, 1, 3, 4, 5];
const FEED_ROW = 2;
const BAUD_MS = 150; // 50 baud: about 6.7 characters a second, the speed of a real telex line
const PAUSE_MS = 1800; // between messages

type Step = Column & { last: boolean };

/**
 * A teleprinter at work: each message is punched into paper tape in ITA2, column by column, and
 * printed underneath. Runs only while on screen; stands still for reduced motion; can be paused.
 */
export function TelexTicker({ messages }: { messages: string[] }) {
  const [started, setStarted] = useState(false);
  const program = useMemo<Step[]>(
    () =>
      (started ? messages : messages.slice(0, 1)).flatMap((m) => {
        const cols = encode(`${m}   `);
        return cols.map((c, i) => ({ ...c, last: i === cols.length - 1 }));
      }),
    [messages, started],
  );
  const first = useMemo(() => program.findIndex((c) => c.last) + 1, [program]);
  // Server and first paint show the first message already punched.
  const [count, setCount] = useState(first);
  const [width, setWidth] = useState(1440);
  const [visible, setVisible] = useState(false);
  const [still, setStill] = useState(false); // prefers reduced motion
  const [override, setOverride] = useState<boolean | null>(null); // Play or Pause pressed
  const box = useRef<HTMLDivElement>(null);
  const counter = useRef(first);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => {
      setVisible(e.isIntersecting);
      if (e.isIntersecting) setStarted(true);
    });
    io.observe(el);
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onMotion = () => setStill(mq.matches);
    onMotion();
    mq.addEventListener('change', onMotion);
    return () => {
      io.disconnect();
      mq.removeEventListener('change', onMotion);
    };
  }, []);

  useEffect(() => {
    const el = box.current;
    if (!el || !visible) return;
    const ro = new ResizeObserver(([e]) => setWidth(Math.round(e.contentRect.width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, [visible]);

  const wanted = override ?? !still;
  const running = visible && wanted;
  useEffect(() => {
    if (!running) return;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      const n = counter.current;
      counter.current = n + 1;
      setCount(n + 1);
      timer = setTimeout(tick, program[n % program.length].last ? PAUSE_MS : BAUD_MS);
    };
    timer = setTimeout(tick, BAUD_MS);
    return () => clearTimeout(timer);
  }, [running, program]);

  // The print head sits at 80% of the width, but always left of the label and Pause button.
  const head = Math.max(COL * 8, Math.round(Math.min(width * 0.8, width - 250) / COL) * COL);
  const shown = Math.ceil(head / COL) + 1;
  const from = Math.max(0, count - shown);
  const cols = [];
  for (let i = from; i < count; i++) cols.push({ i, ...program[i % program.length] });
  const y = (row: number) => TOP + row * ROW;
  // One path and two text runs replace hundreds of React/SVG nodes. Each character still has
  // its original tape coordinate, including blanks and the letter/figure shift columns.
  const holes = cols.flatMap((c) => CHANNEL_ROWS.flatMap((row, bit) => {
    if (c.code[bit] !== '1') return [];
    const x = c.i * COL + COL / 2;
    return [`M${x - 3.3},${y(row)}a3.3,3.3 0 1,0 6.6,0a3.3,3.3 0 1,0 -6.6,0`];
  })).join('');
  const shifts = cols.filter((c) => c.shift);

  return (
    <div className={`${s.strip} no-print`} ref={box}>
      <svg className={s.svg} width="100%" height={HEIGHT} aria-hidden="true">
        <defs>
          <pattern id="tlx-feed" width={COL} height={HEIGHT} patternUnits="userSpaceOnUse" x={head % COL}>
            <circle cx={COL / 2} cy={y(FEED_ROW)} r="1.5" className={s.feed} />
          </pattern>
        </defs>
        <rect width="100%" height={TAPE_H} fill="url(#tlx-feed)" />
        {/* Everything punched so far, shifted so the newest column sits just left of the head. */}
        <g transform={`translate(${head - count * COL} 0)`}>
          <path d={holes} className={s.hole} />
          <text x={cols.map((c) => c.i * COL + COL / 2).join(' ')} y={TEXT_Y} className={s.char} xmlSpace="preserve">
            {cols.map((c) => c.char.trim() ? c.char : ' ').join('')}
          </text>
          {shifts.length > 0 && <text x={shifts.map((c) => c.i * COL + COL / 2).join(' ')} y={TEXT_Y} className={s.shift}>
            {shifts.map((c) => c.shift === 'figs' ? '↑' : '↓').join('')}
          </text>}
        </g>
        <line x1={head} x2={head} y1={4} y2={TAPE_H - 4} className={s.head} />
        <rect x={head + 3} y={TEXT_Y - 10} width="7" height="12" className={running ? s.caret : s.caretStill} />
      </svg>
      <div className={s.meta}>
        <span className={s.baud}>50 baud · ITA2</span>
        <button type="button" className={s.pause} onClick={() => setOverride(!wanted)}>
          {wanted ? 'Pause' : 'Play'}
          <span className="visually-hidden"> the telex tape</span>
        </button>
      </div>
      <p className="visually-hidden">{messages.join(' ')}</p>
    </div>
  );
}
