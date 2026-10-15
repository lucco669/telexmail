'use client';

import Image from 'next/image';
import { useEffect, useReducer, useRef, useState, type CSSProperties } from 'react';
import { encode } from '@/lib/ita2';
import { initialShift, multiplier, runMetrics, SHIFT_SECONDS, shiftReducer, stations, suggestedLine, type Mode } from '@/lib/night-shift';
import { TelexAudio } from '@/lib/night-shift-audio';
import type { LeaderboardEntry } from '@/lib/night-shift-ranking';
import { NightShiftScene } from './NightShiftScene';
import { NightShiftLeaderboard } from './NightShiftLeaderboard';
import s from './NightShift.module.css';

const RECORD_KEY = 'telex-night-shift-best-v1';

function Tape({ text }: { text: string }) {
  const columns = encode(text).slice(-36);
  return <svg className={s.tape} viewBox="0 0 540 54" preserveAspectRatio="none" role="img" aria-label="Your message encoded as five-channel ITA2 paper tape">
    <rect width="540" height="54" fill="#ded4b7" />
    {Array.from({ length: 36 }, (_, i) => <circle key={`feed-${i}`} cx={i * 15 + 7.5} cy="23" r="1.3" fill="#1a2528" />)}
    {columns.map((column, i) => <g key={i}>{[7, 15, 31, 39, 47].map((y, bit) => column.code[bit] === '1' ? <circle key={bit} cx={i * 15 + 7.5} cy={y} r="2.7" fill="#1a2528" /> : null)}</g>)}
  </svg>;
}

export function NightShift() {
  const [state, dispatch] = useReducer(shiftReducer, initialShift);
  const [mode, setMode] = useState<Mode>('shift');
  const [sound, setSound] = useState(false);
  const [best, setBest] = useState(0);
  const [newRecord, setNewRecord] = useState(false);
  const [effects, setEffects] = useState(true);
  const [sceneVisible, setSceneVisible] = useState(false);
  const [starting, setStarting] = useState(false);
  const [rankedToken, setRankedToken] = useState<string | null>(null);
  const [operator, setOperator] = useState('');
  const [publishing, setPublishing] = useState(false);
  const [publishMessage, setPublishMessage] = useState('');
  const [publishedId, setPublishedId] = useState<string | null>(null);
  const [publishedEntry, setPublishedEntry] = useState<LeaderboardEntry | null>(null);
  const [boardRefresh, setBoardRefresh] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const resume = useRef<HTMLButtonElement>(null);
  const resultHeading = useRef<HTMLHeadingElement>(null);
  const nextButton = useRef<HTMLButtonElement>(null);
  const startButton = useRef<HTMLButtonElement>(null);
  const returningToTitle = useRef(false);
  const startingSession = useRef(false);
  const audio = useRef<TelexAudio | null>(null);
  const clock = useRef({ base: 0, started: 0 });
  const previousDelivered = useRef(0);
  const recordLoaded = useRef(false);
  const circuit = state.circuits[state.active];
  const playing = state.phase === 'playing';
  const inSession = state.phase !== 'ready';
  const sending = circuit?.sendingUntil !== null && circuit?.sendingUntil !== undefined;
  const complete = circuit?.draft === circuit?.message;
  const remaining = Math.max(0, SHIFT_SECONDS - Math.floor(state.now));
  const tapeText = circuit?.draft ?? 'NIGHT SHIFT 1965';
  const nextLine = suggestedLine(state);
  const metrics = runMetrics(state);
  const seconds = (ms: number | null) => ms === null ? '—' : `${(ms / 1000).toFixed(2)}s`;
  const timestamp = () => playing ? clock.current.base + (performance.now() - clock.current.started) / 1000 : state.now;

  function tone(kind: 'key' | 'send' | 'delivered') {
    if (!sound) return;
    if (kind === 'delivered') audio.current?.bell();
    else audio.current?.key();
  }

  function toggleSound() {
    if (!sound) {
      try {
        audio.current ??= new TelexAudio();
      } catch { return; }
    }
    setSound(!sound);
  }

  async function start() {
    if (starting) return;
    setStarting(true); setRankedToken(null); setPublishedId(null); setPublishedEntry(null); setPublishMessage('');
    let seed = Date.now();
    if (mode === 'shift') {
      try {
        const response = await fetch('/api/night-shift/session', { method: 'POST', signal: AbortSignal.timeout(4000) });
        if (response.ok) { const session = await response.json(); seed = session.seed; setRankedToken(session.token); }
      } catch { /* A local shift is still playable while the online desk is unavailable. */ }
    }
    startingSession.current = true;
    setNewRecord(false);
    previousDelivered.current = 0;
    dispatch({ type: 'start', mode, seed });
    setStarting(false);
  }

  function send() {
    if (!playing || !complete || sending) return;
    tone('send');
    dispatch({ type: 'send', seconds: encode(circuit.message).length * 0.15, at: timestamp() });
  }

  useEffect(() => {
    try {
      const saved = Number(localStorage.getItem(RECORD_KEY));
      if (Number.isSafeInteger(saved) && saved > 0) setBest(saved);
    } catch { /* The game also works when browser storage is unavailable. */ }
    recordLoaded.current = true;
    try { setOperator(localStorage.getItem('telex-night-shift-operator') ?? ''); } catch {}
    return () => { audio.current?.dispose(); };
  }, []);

  useEffect(() => {
    let scrollFrame: number | undefined;
    const showDesk = () => {
      // Align after the title/console layout has settled and focus has moved.
      scrollFrame = requestAnimationFrame(() => root.current?.scrollIntoView({ block: 'start', behavior: 'instant' }));
    };
    if (state.phase === 'ready' && returningToTitle.current) {
      returningToTitle.current = false;
      startButton.current?.focus({ preventScroll: true });
      showDesk();
    }
    if (state.phase === 'playing' && startingSession.current) {
      startingSession.current = false;
      showDesk();
    }
    if (state.phase === 'playing') {
      if (sending) (nextButton.current ?? root.current)?.focus({ preventScroll: true });
      else input.current?.focus({ preventScroll: true });
    }
    if (state.phase === 'paused') { resume.current?.focus({ preventScroll: true }); showDesk(); }
    if (state.phase === 'finished') { resultHeading.current?.focus({ preventScroll: true }); showDesk(); }
    return () => { if (scrollFrame !== undefined) cancelAnimationFrame(scrollFrame); };
  }, [state.phase, state.active, circuit?.ticket, sending]);

  useEffect(() => {
    if (!playing) return;
    clock.current = { base: state.now, started: performance.now() };
    const timer = window.setInterval(() => {
      dispatch({ type: 'advance', at: clock.current.base + (performance.now() - clock.current.started) / 1000 });
    }, 100);
    const pause = () => dispatch({ type: 'pause', at: clock.current.base + (performance.now() - clock.current.started) / 1000 });
    const visibility = () => { if (document.hidden) pause(); };
    document.addEventListener('visibilitychange', visibility);
    const observer = new IntersectionObserver(([entry]) => { if (!entry.isIntersecting) pause(); });
    if (root.current) observer.observe(root.current);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', visibility);
      observer.disconnect();
    };
  }, [playing]);

  useEffect(() => {
    const observer = new IntersectionObserver(([entry]) => setSceneVisible(entry.isIntersecting && !document.hidden));
    observer.observe(root.current!);
    const visibility = () => setSceneVisible(!document.hidden && !!root.current && root.current.getBoundingClientRect().bottom > 0 && root.current.getBoundingClientRect().top < innerHeight);
    document.addEventListener('visibilitychange', visibility);
    return () => { observer.disconnect(); document.removeEventListener('visibilitychange', visibility); };
  }, []);

  useEffect(() => {
    audio.current?.setActivity(sound && sceneVisible && (state.phase === 'ready' || playing), state.circuits.flatMap((line, i) => line.sendingUntil !== null ? [i] : []));
  }, [sound, sceneVisible, playing, state.phase, state.circuits]);

  async function publishScore(event: React.FormEvent) {
    event.preventDefault();
    if (!rankedToken || publishing || publishedId) return;
    setPublishing(true); setPublishMessage('Calling the international desk…');
    try {
      const response = await fetch('/api/night-shift/leaderboard', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, signal: AbortSignal.timeout(12000),
        body: JSON.stringify({ token: rankedToken, operator, elapsed: state.now, events: state.events }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error ?? 'Your score could not be published. Try again.');
      setPublishedId(result.id); setPublishedEntry(result.entry ?? null); setPublishMessage('Your shift is on the world board.'); setBoardRefresh((value) => value + 1);
      try { localStorage.setItem('telex-night-shift-operator', operator.trim()); } catch {}
    } catch (error) { setPublishMessage(error instanceof Error ? error.message : 'The desk could not be reached. Try again.'); }
    setPublishing(false);
  }

  useEffect(() => {
    if (state.delivered > previousDelivered.current) tone('delivered');
    previousDelivered.current = state.delivered;
    // Only completed scored shifts count toward the personal record.
    if (state.phase !== 'finished' || state.mode !== 'shift' || !recordLoaded.current || state.score <= best) return;
    setBest(state.score);
    setNewRecord(true);
    try { localStorage.setItem(RECORD_KEY, String(state.score)); } catch { /* Session record still works. */ }
  }, [state.delivered, state.phase, state.mode, state.score, best, sound]);

  const paperProgress = sending ? Math.max(0, Math.min(100, 100 * (1 - (circuit.sendingUntil! - state.now) / (encode(circuit.message).length * 0.15)))) : 0;

  return <><div ref={root} className={s.game} tabIndex={-1} data-phase={state.phase} onKeyDown={(event) => {
    if (event.key === 'Escape' && playing) { event.preventDefault(); dispatch({ type: 'pause', at: timestamp() }); }
    if (event.altKey && /^[123]$/.test(event.key) && playing) { event.preventDefault(); dispatch({ type: 'select', index: Number(event.key) - 1, at: timestamp() }); }
  }}>
    <Image src="/history/night-shift.webp" alt="" fill sizes="(max-width: 1224px) calc(100vw - 32px), 1160px" className={s.scene} />
    {effects && <NightShiftScene active={state.phase === 'ready' || playing} />}
    <div className={s.shade} aria-hidden="true" />
    <div className={s.grain} aria-hidden="true" />
    <div className={s.topbar} inert={state.phase === 'paused' || state.phase === 'finished'}>
      <span className={s.stationLabel}><i aria-hidden="true" /> INTERNATIONAL TELEX / NIGHT DESK</span>
      <div className={s.topControls}>
        <button type="button" aria-pressed={effects} onClick={() => setEffects(!effects)}>Atmosphere {effects ? 'on' : 'off'}</button>
        {inSession && state.phase !== 'finished' && <button type="button" onClick={() => dispatch({ type: playing ? 'pause' : 'resume', ...(playing ? { at: timestamp() } : {}) })}>{playing ? 'Pause' : 'Resume'}</button>}
        <button type="button" aria-pressed={sound} onClick={toggleSound}>Sound {sound ? 'on' : 'off'} <span aria-hidden="true">{sound ? '♪' : '♩'}</span></button>
      </div>
    </div>

    {state.phase === 'ready' ? <div className={s.intro}>
      <div className={s.introCopy}>
        <p className={s.kicker}>AN OPERATOR’S LITTLE OBSESSION</p>
        <h3>Night<br /><em>Shift.</em></h3>
        <p className={s.introText}>The city sleeps.<br />The messages don’t.</p>
        <p className={s.description}>Take the desk. Copy incoming telegrams.<br />Keep three cities connected until dawn.</p>
      </div>
      <div className={s.startPanel}>
        <div className={s.modePicker} role="group" aria-label="Game mode">
          <button type="button" aria-pressed={mode === 'shift'} onClick={() => setMode('shift')}>90-second shift <span>BEAT YOUR BEST</span></button>
          <button type="button" aria-pressed={mode === 'practice'} onClick={() => setMode('practice')}>Quiet practice <span>NO DEADLINES</span></button>
        </div>
        <button ref={startButton} type="button" className={s.primary} disabled={starting} onClick={start}>{starting ? 'Connecting the desk…' : 'Take the night shift'} <span aria-hidden="true">↗</span></button>
        <p className={s.personalBest}>PERSONAL BEST <strong>{best.toLocaleString('en-US')}</strong></p>
      </div>
      <ol className={s.instructions}>
        <li><span>01</span><div><strong>Pick a line</strong><p>{mode === 'practice' ? 'Three cities. Take your time.' : 'Watch the three city deadlines.'}</p></div></li>
        <li><span>02</span><div><strong>Copy the telegram</strong><p>Clean typing builds your multiplier.</p></div></li>
        <li><span>03</span><div><strong>Transmit & switch</strong><p>Press Enter. Switch while the tape sends.</p></div></li>
      </ol>
    </div> : <div className={s.console} inert={!playing}>
      <div className={s.scoreboard}>
        <div><span>SCORE</span><strong>{state.score.toLocaleString('en-US')}</strong></div>
        <div><span>{state.mode === 'practice' ? 'PRACTICE' : 'SHIFT LEFT'}</span><strong className={state.mode === 'shift' && remaining <= 15 ? s.urgent : ''}>{state.mode === 'practice' ? '∞' : `${String(Math.floor(remaining / 60)).padStart(2, '0')}:${String(remaining % 60).padStart(2, '0')}`}</strong></div>
        <div><span>CLEAN STREAK</span><strong>{state.streak}<small> ×{multiplier(state.streak)}</small></strong></div>
        <div><span>{state.mode === 'practice' ? 'DELIVERED' : 'CHANCES'}</span><strong>{state.mode === 'practice' ? state.delivered : `${Math.max(0, 3 - state.missed)} / 3`}</strong></div>
      </div>
      <div className={s.lines} role="group" aria-label="Choose a telex line">
        {state.circuits.map((line, i) => {
          const left = Math.max(0, Math.ceil(line.deadline - state.now));
          const transmitting = line.sendingUntil !== null;
          return <button key={i} type="button" disabled={!playing} className={`${s.line} ${left <= 10 && !transmitting && state.mode === 'shift' ? s.lineUrgent : ''}`} aria-pressed={state.active === i} onClick={() => dispatch({ type: 'select', index: i, at: timestamp() })}>
            <span className={s.lineTop}><span><b>{String(i + 1).padStart(2, '0')}</b> {stations[i].city}</span><i className={transmitting ? s.busyLight : ''} aria-hidden="true" /></span>
            <span className={s.lineStatus}>{transmitting ? 'TRANSMITTING' : state.mode === 'practice' ? 'TELEGRAM WAITING' : `DEADLINE ${left}s`}</span>
            <span className={s.lineMeter} aria-hidden="true"><span style={{ width: `${transmitting || state.mode === 'practice' ? 100 : Math.min(100, left / line.allowance * 100)}%` }} /></span>
          </button>;
        })}
      </div>
      <div className={s.dispatchHint}><p>{sending ? 'This line is busy. Switch cities while the tape transmits.' : 'Finish this copy, transmit it, then move to another waiting city.'}</p>{nextLine !== null && nextLine !== state.active && <button ref={nextButton} type="button" onClick={() => dispatch({ type: 'select', index: nextLine, at: timestamp() })}>Next: {stations[nextLine].city} ↗</button>}</div>
      <div className={s.workbench}>
        <div className={s.paper} key={circuit.ticket}>
          <div className={s.paperHeader}><span>INCOMING TELEGRAM</span><span>NO. {circuit.ticket}</span></div>
          <div className={s.answerback}>TO {stations[state.active].identity}</div>
          <p className={s.message} aria-label={`Telegram to copy: ${circuit.message}`}>
            {Array.from(circuit.message).map((character, i) => <span aria-hidden="true" key={i} className={i < circuit.draft.length ? circuit.draft[i] === character ? s.correct : s.wrong : i === circuit.draft.length ? s.next : undefined}>{character}</span>)}
          </p>
          <div className={s.paperFooter}><span>CHECK ANSWERBACK / COPY EXACTLY</span><span>{circuit.spoiled ? 'CORRECTION MADE' : 'CLEAN COPY BONUS +50'}</span></div>
          {sending && <div className={s.transmitting} style={{ '--progress': `${paperProgress}%` } as CSSProperties}><span>ON THE WIRE</span><strong>{Math.ceil(circuit.sendingUntil! - state.now)}s</strong><p>You can prepare another line while this one prints.</p></div>}
        </div>
        <div className={s.operatorPanel}>
          <label htmlFor="night-shift-input">YOUR TRANSMISSION <span>{sending ? 'LINE BUSY' : `${circuit.draft.length} / ${circuit.message.length}`}</span></label>
          <form onSubmit={(event) => { event.preventDefault(); send(); }}>
            <input ref={input} id="night-shift-input" value={circuit.draft} disabled={!playing || sending} onChange={(event) => { dispatch({ type: 'type', value: event.target.value, at: timestamp() }); if (publishMessage) setPublishMessage(''); tone('key'); }} onPaste={(event) => { if (state.mode === 'shift') { event.preventDefault(); setPublishMessage('Scored shifts use live typing. Paste is available in practice.'); } }} onDrop={(event) => { if (state.mode === 'shift') event.preventDefault(); }} autoComplete="off" autoCorrect="off" autoCapitalize="characters" spellCheck={false} maxLength={circuit.message.length + 5} placeholder="Type the telegram above…" aria-describedby="night-shift-input-help" />
            <button type="submit" className={s.sendButton} disabled={!playing || !complete || sending}>{sending ? 'Transmitting…' : 'Transmit'} <kbd>↵</kbd></button>
          </form>
          <p id="night-shift-input-help" className={s.inputHelp}>{sending ? 'Select another city to keep the messages moving.' : complete ? 'Ready. Press Enter to send it down the wire.' : circuit.message.startsWith(circuit.draft) ? 'Lowercase becomes uppercase. Spaces and numbers count.' : 'A character is off. Backspace to correct your copy.'}</p>
          <div className={s.tapeHousing}><Tape text={tapeText} /><span>50 BAUD / ITA2</span></div>
        </div>
      </div>
      <dl className={s.liveMetrics}><div><dt>Avg. reaction</dt><dd>{seconds(metrics.reactionMs)}</dd></div><div><dt>Avg. writing</dt><dd>{seconds(metrics.writingMs)}</dd></div><div><dt>Typing speed</dt><dd>{metrics.wpm || '—'} <small>WPM</small></dd></div><div><dt>Accuracy</dt><dd>{metrics.accuracy}%</dd></div></dl>
      <div className={s.deskFooter}>
        <div className={s.receipts} aria-label="Recent deliveries">{state.receipts.length ? state.receipts.map((receipt, i) => <p key={`${receipt}-${i}`}>{receipt}</p>) : <p>RECEIPTS WILL PRINT HERE / KEEP THE LINE OPEN</p>}</div>
        <div className={s.shortcuts}><span><kbd>Alt</kbd> + <kbd>1</kbd> <kbd>2</kbd> <kbd>3</kbd> switch lines</span><span><kbd>Esc</kbd> pause</span>{state.mode === 'practice' && <button type="button" onClick={() => dispatch({ type: 'finish', at: timestamp() })}>End practice ↗</button>}</div>
      </div>
      <p className={s.bulletin} role="status" aria-live="polite">{publishMessage || state.bulletin}</p>
    </div>}

    {state.phase === 'paused' && <div className={s.overlay} role="region" aria-label="Night Shift paused">
      <div className={s.pauseCard}><p className={s.kicker}>THE LINES CAN WAIT</p><h3>Desk on hold.</h3><p>Your clocks are paused. Pick up where you left off.</p><button ref={resume} type="button" className={s.primary} onClick={() => dispatch({ type: 'resume' })}>Back to the desk <span aria-hidden="true">↗</span></button><button type="button" className={s.secondary} onClick={() => dispatch({ type: 'finish' })}>Finish this session</button></div>
    </div>}
    {state.phase === 'finished' && <div className={s.overlay} role="region" aria-label="Night Shift results">
      <div className={s.resultCard}>
        <p className={s.kicker}>{state.mode === 'practice' ? 'PRACTICE COMPLETE' : newRecord ? 'A NEW PERSONAL BEST' : state.missed >= 3 ? 'THE LINES GOT AWAY' : 'YOUR SHIFT IS COMPLETE'}</p>
        <h3 ref={resultHeading} tabIndex={-1}>{state.missed >= 3 ? 'Tomorrow’s another night.' : 'Good night, operator.'}</h3>
        <div className={s.resultScore}>{state.score.toLocaleString('en-US')}<span>POINTS ON THE BOARD</span></div>
        <dl className={s.results}><div><dt>Delivered</dt><dd>{state.delivered}</dd></div><div><dt>Clean copies</dt><dd>{state.clean}</dd></div><div><dt>Best streak</dt><dd>{state.bestStreak}</dd></div></dl>
        <dl className={s.resultMetrics}>
          <div><dt>Avg. reaction</dt><dd>{seconds(metrics.reactionMs)}</dd></div><div><dt>Avg. writing</dt><dd>{seconds(metrics.writingMs)}</dd></div>
          <div><dt>Typing speed</dt><dd>{metrics.wpm || '—'} <small>WPM</small></dd></div><div><dt>Accuracy</dt><dd>{metrics.accuracy}%</dd></div>
        </dl>
        <div className={s.cityResults}>{stations.map((station, i) => {
          const telegrams = state.telegrams.filter((item) => item.city === i);
          const city = runMetrics({ ...state, telegrams });
          return <p key={station.code}><strong>{station.city}</strong><span>{telegrams.length} sent</span><span>{seconds(city.reactionMs)} react</span><span>{city.wpm || '—'} WPM</span></p>;
        })}</div>
        <p className={s.timingNote}>Averages use delivered telegrams. Reaction starts when you first see the copy; writing counts active work on that line. Pauses and time in other cities are excluded.</p>
        {state.mode === 'shift' && rankedToken && state.delivered > 0 && state.finishReason !== 'retired' && <form className={s.publishForm} onSubmit={publishScore}>
          <label htmlFor="night-shift-operator">Operator name <span>PUBLIC ON THE WORLD BOARD</span></label>
          <div><input id="night-shift-operator" value={operator} minLength={2} maxLength={20} onChange={(event) => setOperator(event.target.value)} placeholder="Your callsign" required disabled={publishing || !!publishedId} autoComplete="nickname" /><button type="submit" disabled={publishing || !!publishedId}>{publishedId ? 'Published ✓' : publishing ? 'Publishing…' : 'Publish this shift ↗'}</button></div>
          <p>Your chosen name and run statistics will be visible to everyone.</p>
        </form>}
        {state.mode === 'shift' && !rankedToken && <p className={s.timingNote}>Online ranking was unavailable when this shift started. Your personal best still stays on this device.</p>}
        {state.mode === 'shift' && rankedToken && state.finishReason === 'retired' && <p className={s.timingNote}>This shift ended early. Complete the timer or reach three missed deadlines to enter the world board.</p>}
        {publishMessage && <p className={s.publishStatus} role="status">{publishMessage}</p>}
        <button type="button" className={s.primary} disabled={starting || publishing} onClick={start}>{starting ? 'Connecting the desk…' : `One more ${state.mode === 'practice' ? 'practice' : 'shift'}`} <span aria-hidden="true">↻</span></button>
        <button type="button" className={s.secondary} disabled={publishing} onClick={() => { returningToTitle.current = true; dispatch({ type: 'reset' }); }}>Return to title / change mode</button>
      </div>
    </div>}
    <noscript><p className={s.noScript}>Enable JavaScript to play Night Shift.</p></noscript>
  </div><NightShiftLeaderboard refresh={boardRefresh} published={publishedEntry} /></>;
}
