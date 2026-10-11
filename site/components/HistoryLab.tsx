'use client';

import { useState } from 'react';
import { encode } from '@/lib/ita2';
import { historySources, milestones } from '@/lib/history';
import s from './HistoryLab.module.css';

const filters = [
  ['all', 'All milestones'], ['wire', 'Telegraph & telex'], ['network', 'Email & networks'],
] as const;

export function HistoryTimeline() {
  const [filter, setFilter] = useState<(typeof filters)[number][0]>('all');
  const shown = milestones.filter((m) => filter === 'all' || m.branch === filter || m.branch === 'both');
  return (
    <div>
      <div className={s.controls} role="group" aria-label="Timeline branch">
        {filters.map(([id, label]) => (
          <button type="button" key={id} aria-pressed={filter === id} onClick={() => setFilter(id)}>{label}</button>
        ))}
      </div>
      <p className={s.count} role="status">{shown.length} milestones · selected highlights, not an exhaustive chronology</p>
      <ol className={s.timeline}>
        {shown.map((m) => {
          const source = historySources.find((item) => item.id === m.source)!;
          return (
            <li key={m.title}>
              <span className={s.date}>{m.date}</span>
              <div><h3>{m.title}</h3><p>{m.text} <a href={source.url} aria-label={`Source: ${source.title}`} title={source.title}>↗</a></p></div>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

const examples = ['TELEX 2026', 'HELLO WORLD', 'E3E', 'ORDER 24 BOXES'];
const permitted = /^[A-Z0-9 .,:?'\/+=()\-]$/;
const rows = [19, 35, 67, 83, 99];

export function PaperTapeLab() {
  const [input, setInput] = useState(examples[0]);
  const [selected, setSelected] = useState(0);
  const upper = input.toUpperCase();
  const normalized = [...upper].map((char) => permitted.test(char) ? char : ' ').join('');
  const columns = encode(normalized);
  const index = Math.min(selected, Math.max(0, columns.length - 1));
  const column = columns[index];
  const shifts = columns.filter((c) => c.shift).length;
  const width = Math.max(640, columns.length * 36 + 32);
  const describe = (i: number) => {
    const c = columns[i];
    return c.shift === 'figs' ? 'FIGS: switch to figures' : c.shift === 'ltrs' ? 'LTRS: switch to letters' : c.char === ' ' ? 'Space' : c.char;
  };
  function update(value: string) { setInput(value); setSelected(0); }
  return (
    <div className={s.lab}>
      <div className={s.labHeading}><span className={s.label}>THE PAPER-TAPE WORKBENCH</span><span className={s.label}>ITA2 / 50 BAUD</span></div>
      <label className={s.inputLabel} htmlFor="tape-message">Type a short message</label>
      <input id="tape-message" className={s.input} value={input} maxLength={60} onChange={(e) => update(e.target.value)} aria-describedby="tape-help" autoComplete="off" spellCheck={false} />
      <p id="tape-help" className={s.help}>Letters become uppercase. This compact alphabet supports A–Z, digits, spaces and . , : ? &apos; / + = ( ) -. Other characters become spaces.</p>
      <div className={s.examples} role="group" aria-label="Example messages">
        <span>Try</span>{examples.map((text) => <button key={text} type="button" onClick={() => update(text)}>{text}</button>)}
      </div>
      <div className={s.tapeScroll} tabIndex={0} role="region" aria-label="Encoded paper tape; scroll horizontally for longer messages">
        <svg viewBox={`0 0 ${width} 151`} width={width} height="151" className={s.tape} aria-hidden="true">
          <rect x="0" y="0" width={width} height="118" className={s.paper} />
          {columns.map((c, i) => (
            <g key={i}>
              {index === i && <rect x={i * 36 + 2} y="3" width="32" height="112" className={s.selectedColumn} />}
              {rows.map((y, bit) => c.code[bit] === '1' ? <circle key={y} cx={i * 36 + 18} cy={y} r="5" className={s.hole} /> : null)}
              <circle cx={i * 36 + 18} cy="51" r="2.5" className={s.hole} />
              <text x={i * 36 + 18} y="141" textAnchor="middle" className={c.shift ? s.shiftCharacter : s.character}>{c.shift?.toUpperCase() ?? (c.char === ' ' ? '␣' : c.char)}</text>
            </g>
          ))}
        </svg>
      </div>
      <div className={s.columnPicker} role="group" aria-label="Inspect a tape column">
        {columns.map((c, i) => <button key={i} type="button" aria-label={`Column ${i + 1}: ${describe(i)}`} aria-pressed={index === i} onClick={() => setSelected(i)}>{c.shift?.toUpperCase() ?? (c.char === ' ' ? '␣' : c.char)}</button>)}
      </div>
      <div className={s.inspection} role="status">
        {column ? <><strong>{describe(index)}</strong><code>{column.code}</code><span>Channel 1 → 5 · {column.shift ? 'changes mode; prints nothing' : '1 = a hole, 0 = paper'}</span></> : <span>Enter a message to see its tape.</span>}
      </div>
      <dl className={s.stats}>
        <div><dt>Printed characters</dt><dd>{normalized.length}</dd></div>
        <div><dt>Shift columns</dt><dd>{shifts}</dd></div>
        <div><dt>Time on the wire</dt><dd>≈ {(columns.length * 0.15).toFixed(2)} s</dd></div>
      </dl>
      <p className={s.help}>Starts in letters mode. Tape shows data bits and feed holes; start and stop signals travel on the wire. Timing assumes 1 start + 5 data + 1½ stop units at 50 baud, including shifts and excluding connection setup.</p>
      <noscript><p className={s.help}>The example tape is visible without JavaScript. Enable JavaScript to encode your own message and inspect columns.</p></noscript>
    </div>
  );
}

const routes = {
  telex: [
    { title: 'Prepare', place: 'Sending office', text: 'Write the message, or prepare a punched tape before making the call. The tape can be read back at the line’s steady speed.' },
    { title: 'Connect', place: 'Telex exchange', text: 'Call the subscriber’s telex number. The switching network establishes a connection to the receiving terminal.' },
    { title: 'Identify', place: 'Answerback', text: 'Request the remote machine’s answerback and check the returned identifier. This identifies a terminal; it does not prove a particular person is at the keyboard.' },
    { title: 'Transmit', place: 'Receiving teleprinter', text: 'Typed characters or tape contents pass over the established connection and print at the other end. A powered, ready terminal can receive without an operator typing a reply.' },
    { title: 'Close', place: 'A paper record', text: 'A further answerback check can confirm that the connection is still to the expected terminal. Clear the call and retain the printed record.' },
  ],
  email: [
    { title: 'Compose', place: 'Mail client', text: 'Your client builds headers and a body, with MIME parts when needed. The visible To and From fields belong to the message; the transport has its own envelope addresses.' },
    { title: 'Submit', place: 'Sending server', text: 'The client hands the message to its outgoing service. Acceptance means that service has taken responsibility for it; it does not mean the recipient has read it.' },
    { title: 'Relay', place: 'SMTP between servers', text: 'The sending system finds the destination’s mail servers through DNS and transfers the message using SMTP. Temporary problems can lead to queueing and retries.' },
    { title: 'Deliver', place: 'Recipient’s mailbox', text: 'The destination accepts and processes the message. Filtering can put it in a folder, quarantine it or reject it. The recipient does not have to be online.' },
    { title: 'Read & sync', place: 'IMAP or webmail', text: 'A mail client accesses the mailbox. IMAP synchronizes messages, flags and folders; a webmail interface accesses the service through the browser. Reading is a separate event from delivery.' },
  ],
};

export function MessageJourney() {
  const [kind, setKind] = useState<'telex' | 'email'>('telex');
  const [step, setStep] = useState(0);
  const active = routes[kind][step];
  return (
    <div className={s.journey}>
      <div className={s.controls} role="group" aria-label="Message journey">
        <button type="button" aria-pressed={kind === 'telex'} onClick={() => { setKind('telex'); setStep(0); }}>A telex call</button>
        <button type="button" aria-pressed={kind === 'email'} onClick={() => { setKind('email'); setStep(0); }}>An email</button>
      </div>
      <ol className={s.steps} aria-label={`${kind === 'telex' ? 'Telex' : 'Email'} stages`}>
        {routes[kind].map((item, i) => <li key={item.title}><button type="button" aria-pressed={step === i} onClick={() => setStep(i)}><span>{String(i + 1).padStart(2, '0')}</span>{item.title}</button></li>)}
      </ol>
      <div className={s.journeyDetail} role="status"><span className={s.label}>{active.place}</span><h3>{active.title}</h3><p>{active.text}</p></div>
      <p className={s.help}>Simplified journeys. Real telex services also developed gateways and store-and-forward facilities; email delivery can involve several relays.</p>
    </div>
  );
}
