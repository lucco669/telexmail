import Image from 'next/image';
import { PageLink as Link } from '@/components/PageNavigation';
import { HistoryTimeline, MessageJourney, PaperTapeLab } from '@/components/HistoryLab';
import { JsonLd } from '@/components/JsonLd';
import { NightShift } from '@/components/NightShift';
import { PrintButton } from '@/components/PrintButton';
import { Shot } from '@/components/Shot';
import { chapters, glossary, historySources, type HistorySourceId } from '@/lib/history';
import { pages } from '@/lib/pages';
import { breadcrumbLd, pageMetadata } from '@/lib/seo';
import { site } from '@/lib/site';
import s from './page.module.css';

export const metadata = {
  ...pageMetadata(pages.history),
  keywords: ['telex history', 'email history', 'teleprinter', 'ITA2', 'Baudot code', 'telegraph', 'SMTP', 'history of messaging'],
};

function Cite({ ids }: { ids: HistorySourceId[] }) {
  return <sup className={s.cite}>{ids.map((id) => {
    const index = historySources.findIndex((source) => source.id === id);
    const source = historySources[index];
    return <a key={id} href={source.url} aria-label={`Source ${index + 1}: ${source.title}`} title={`${source.publisher}: ${source.title}`}>[{index + 1}]</a>;
  })}</sup>;
}

function SectionTitle({ number, id, title, deck }: { number: string; id: string; title: string; deck?: string }) {
  return <header className={s.sectionHead}><span className={s.number}>{number}</span><h2 id={id}>{title}</h2>{deck && <p>{deck}</p>}</header>;
}

const relatives: { name: string; kind: string; text: string; sources: HistorySourceId[] }[] = [
  { name: 'Telegram', kind: 'A service, not a machine', text: 'A customer hands a message to an operator or service. The network carries it; the service delivers it to the addressee. Telex instead gives a subscriber direct access to a connected terminal.', sources: ['morse', 'itelex-basics'] },
  { name: 'Fax', kind: 'Send the appearance of a page', text: 'Fax communicates a document as an image. A signature, a sketch or a form can travel without being recast as characters. Telex communicates coded text, which is a different kind of representation.', sources: ['fax'] },
  { name: 'RTTY', kind: 'The teleprinter takes to radio', text: 'Radioteletype carries text by radio. Classic amateur RTTY uses a five-bit alphabet related to telex; it does not require a subscriber telephone-style exchange. The relationship is in the code and printing tradition.', sources: ['rtty'] },
  { name: 'BBS & Usenet', kind: 'From mailboxes to communities', text: 'Bulletin boards offer messages and files on a host that callers reach by modem. Usenet distributes articles through newsgroups; NNTP lets systems exchange and readers retrieve those articles. A public discussion is different from a private inbox.', sources: ['bbs', 'nntp'] },
  { name: 'Teletext & videotex', kind: 'Similar names, different purposes', text: 'Teletext broadcasts pages of information with a television signal. Videotex systems such as Prestel and Minitel use an interactive connection to retrieve services. Neither term is another name for the telex subscriber network.', sources: ['teletext', 'chm'] },
  { name: 'SMS', kind: 'Address a phone, leave a message', text: 'SMS makes brief written communication part of mobile telephony. Its phone-number addressing feels closer to a dialled destination; its message service separates sending from the recipient reading, as email does.', sources: ['sms'] },
  { name: 'IRC & instant messaging', kind: 'Conversation becomes the interface', text: 'IRC organizes conversation around nicknames and channels. Presence and rapid replies make chat feel like a shared room. Email’s addressed message and durable mailbox support a different rhythm, even when delivery is quick.', sources: ['irc'] },
  { name: 'Federated chat', kind: 'Independent services, common rules', text: 'XMPP uses domain-based addresses and open specifications. Matrix defines clients, shared rooms and communication between homeservers. The resemblance to email is architectural: separate operators can communicate. Their protocols and conversation models remain distinct.', sources: ['xmpp', 'matrix'] },
  { name: 'RCS', kind: 'The phone-number conversation expands', text: 'Rich Communication Services adds capabilities such as group chat, media sharing and read receipts to operator-based messaging. Its relationship to SMS is closer than its relationship to email. Supported capabilities depend on the participating services and clients.', sources: ['rcs'] },
  { name: 'Encrypted messengers', kind: 'The reader becomes the security boundary', text: 'Signal says its conversations are always encrypted end to end. This changes the trust question: protection applies to the conversation’s content, beyond merely securing a connection to a provider. Addressing, delivery and device availability still have to be solved.', sources: ['signal'] },
  { name: 'Team chat & threads', kind: 'Keep replies with their context', text: 'Slack threads group replies beneath a particular message. Email uses reply headers and clients assemble conversation views. Both help preserve context, but a thread inside a workspace and a chain of independently delivered emails are different structures.', sources: ['threads', 'format'] },
];

export default function HistoryPage() {
  const sourceGroups = [...new Set(historySources.map((source) => source.group))];
  return (
    <article className={s.page}>
      <JsonLd items={[breadcrumbLd(pages.history), {
        '@type': 'Article', '@id': `${site.url}/history#article`, headline: 'Before the inbox, there was telex.',
        description: pages.history.description, inLanguage: 'en', url: `${site.url}/history`,
        image: `${site.url}/history/teleprinter.webp`, author: { '@type': 'Organization', name: site.name },
        about: ['Telex', 'Telegraphy', 'Email', 'Written communication'],
        citation: historySources.map((source) => source.url),
      }]} />

      <header className={`container ${s.hero}`}>
        <nav className={s.breadcrumb} aria-label="Breadcrumb"><ol><li><Link href="/">Telex</Link></li><li aria-current="page">History</li></ol></nav>
        <div className={s.heroText}>
          <p className={s.eyebrow}>FIELD NOTES / COMMUNICATION, THEN & NOW</p>
          <h1>Before the inbox,<br />there was telex.</h1>
          <p className={s.lead}>A keyboard. A connection. A written record. The long story of how we learned to send words across the world—and what those early machines still have to teach us.</p>
          <div className={s.heroActions}><a href="#timeline" className="btn btn-primary">Explore the story <span aria-hidden="true">↓</span></a><a href="#night-shift" className={s.textLink}>Play Night Shift ↗</a><a href="#tape" className={s.textLink}>Try the paper tape ↗</a></div>
        </div>
        <figure className={s.heroFigure}>
          <Image src="/history/teleprinter.webp" width={1536} height={1024} alt="Illustration of a mid-century mechanical teleprinter with a paper roll, dark keys and a curling strip of punched tape on an ivory desk." sizes="(max-width: 1224px) calc(100vw - 32px), 1160px" loading="eager" fetchPriority="high" />
          <figcaption><span>01 / THE WRITING MACHINE</span><span>Generated illustration · an imagined teleprinter, not an archival photograph.</span></figcaption>
        </figure>
        <div className={s.heroFacts}><p><strong>1790s → today</strong><span>Two centuries of sending words</span></p><p><strong>5 bits</strong><span>The classic telex alphabet</span></p><p><strong>One continuing idea</strong><span>Leave something the other end can read</span></p></div>
      </header>

      <section className={`container ${s.gameSection} no-print`} aria-labelledby="night-shift">
        <div className={s.gameHeading}><div><p className={s.eyebrow}>AN OPERATOR’S BREAK / A PLAYABLE LITTLE FICTION</p><h2 id="night-shift">Your turn at the keyboard.</h2></div><p>Step into the dispatch room.<br /> There are still words to send.</p></div>
        <NightShift />
        <p className={s.gameCaption}>A fictional arcade challenge with generated artwork. Telegrams and answerbacks are invented; deadlines and scoring are game rules. Transmission uses the same ITA2 encoder as the workbench, at 50 baud. Your personal best stays in this browser; choose to publish a scored shift to join the shared world board.</p>
      </section>

      <div className={`container ${s.readingLayout}`}>
        <aside className={`${s.contents} no-print`}>
          <nav aria-label="In this history"><p className={s.eyebrow}>IN THIS STORY</p><ol>{chapters.map(([id, label], i) => <li key={id}><a href={`#${id}`}><span>{String(i + 1).padStart(2, '0')}</span>{label}</a></li>)}</ol></nav>
          <p className={s.contentsNote}>Read straight through,<br />or follow a thread.</p>
          <PrintButton label="Print this story" />
        </aside>

        <div className={s.articleBody}>
          <div className={s.opening}><p>The inbox did not arrive fully formed. Long before email, people were already solving familiar problems: how to address a distant recipient, check the connection, send text efficiently, and keep a record.</p><p>Telex brought those problems together in a particularly tangible way. You could type at one machine and see your words printed at another. Email grew from computer mailboxes and networks, alongside this older world. The connections here are sometimes technical, sometimes similarities of purpose; this is a family of ideas, not one unbroken chain of inventions.</p></div>

          <section className={s.section} aria-labelledby="timeline">
            <SectionTitle number="01" id="timeline" title="A timeline of messages" deck="The systems overlap. New networks do not switch the old ones off overnight." />
            <HistoryTimeline />
          </section>

          <section className={s.section} aria-labelledby="before">
            <SectionTitle number="02" id="before" title="First, separate the message from the journey." />
            <div className={s.prose}>
              <p>A letter makes the journey with its information. A signalling system lets the information travel while the original stays put. Optical telegraphs make that distinction visible: an operator reads a distant tower’s arm positions, then repeats them for the next tower. Chappe’s Paris–Lille line began operating in 1794. It was a network of people, sightlines and codes.<Cite ids={['chappe']} /></p>
              <p>Electrical telegraphy replaces the sightline with a wire. Cooke and Wheatstone’s 1837 apparatus uses needles to indicate letters; the Morse–Vail system’s famous American demonstration follows in 1844. Morse code makes characters into patterns with different lengths. A trained operator supplies the interpretation.<Cite ids={['wheatstone', 'morse']} /></p>
              <p>Cables make this a global undertaking. The successful Atlantic connection of 1866 follows earlier attempts and helps lay the foundation for worldwide cable networks. Speed changes what a distant correspondent can be: someone you can coordinate with during a working day, rather than someone you wait weeks to hear from.<Cite ids={['cable']} /></p>
              <p>Printing telegraphs move another part of the work into the machine. Baudot’s system uses five-unit character codes and a special keyboard. Later teleprinter systems bring typewriter-style input and automatic printing into everyday communication. The receiver can leave a readable record instead of asking someone to decode every signal.<Cite ids={['baudot', 'telex1932']} /></p>
            </div>
            <div className={s.distinction}><span className={s.eyebrow}>THREE DIFFERENT THINGS</span><dl><div><dt>Telegraph</dt><dd>The broad technology of sending coded signals over distance.</dd></div><div><dt>Teleprinter</dt><dd>The machine that turns transmitted character codes into printed text.</dd></div><div><dt>Telex</dt><dd>The switched subscriber service that connects terminals to one another.</dd></div></dl></div>
          </section>

          <section className={s.section} aria-labelledby="telex">
            <SectionTitle number="03" id="telex" title="The world at the other end of a keyboard." deck="Telex made the teleprinter an addressable correspondent." />
            <div className={s.prose}>
              <p>The name is associated with <em>teleprinter exchange</em>. Rather than asking a telegraph office to relay every message, a subscriber can connect to another subscriber’s terminal. British telex service is described in a contemporary report from 1932; networks develop and interconnect across national systems.<Cite ids={['telex1932', 'itelex-basics']} /></p>
              <p>A typical exchange begins with a number and a connection, followed by an <strong>answerback</strong>: an automatic identifier from the remote terminal. The operator checks it, transmits the message, checks again and ends the call. The identifier helps establish which machine is connected. It is not a modern cryptographic signature, and it says nothing by itself about who has read the paper.<Cite ids={['f60', 's-series']} /></p>
              <p>A prepared tape lets an operator write and check a message before occupying the connection. Its holes store the characters physically; a reader feeds them into the transmitter. A receiving terminal can print while its human correspondent is elsewhere. This combination—an address, machine identity and a durable record—makes the system useful for organizational correspondence.<Cite ids={['itelex-basics', 's-series']} /></p>
            </div>
            <figure className={s.transcript}><div className={s.transcriptTop}><span>02 / AN IMAGINED OFFICE EXCHANGE</span><span>ILLUSTRATIVE TEXT</span></div><pre>{'48216 NORTH G\n73902 SOUTH G\n\nATTN SHIPPING\nPLEASE CONFIRM 24 BOXES READY\nFOR COLLECTION MONDAY.\n\nCONFIRMED. COLLECTION 0900.\n\n73902 SOUTH G'}</pre><figcaption>The numbers and names are fictional. The opening identities and closing check show the role of answerback; this is not a transcription of a real telex or a complete signalling log.</figcaption></figure>
            <MessageJourney />
            <p className={s.caption}>Follow each stage to compare a classic telex connection with a simplified internet mail journey.<Cite ids={['f60', 'itelex-basics', 'architecture', 'smtp', 'format', 'imap']} /></p>
            <details className={s.detail}><summary>Why 50 baud does not mean 50 characters a second</summary><p>A familiar telex signalling rate is 50 baud. Five data bits identify a code combination, but each character also needs a start signal and a stop interval. With 7½ signal units per character, 50 ÷ 7.5 gives about 6.7 characters a second, or about 400 a minute. Mode shifts and control characters occupy transmission time too. Other speeds and terminal types existed.<Cite ids={['s-series']} /></p></details>
          </section>

          <section className={s.section} aria-labelledby="tape">
            <SectionTitle number="04" id="tape" title="Five bits, a whole language." deck="A small alphabet turns a mechanical limitation into a clever piece of state." />
            <div className={s.prose}><p>Five bits allow 32 combinations. ITA2 fits more than 32 printable characters by giving many combinations two meanings. <strong>LTRS</strong> selects letters; <strong>FIGS</strong> selects figures and punctuation. The same holes can mean E in letters mode or 3 in figures mode. The receiver remembers which mode it is in.<Cite ids={['ita2']} /></p><p>On paper tape, the five data channels run alongside a smaller row of feed holes. The feed holes move the paper; they are not a sixth data bit. This workbench uses the site’s real ITA2 encoder. Choose <strong>E3E</strong> to see two mode changes around three printed characters. No animation is needed: the mechanism is in the pattern.</p></div>
            <PaperTapeLab />
            <p className={s.caption}>National figure assignments varied. This demonstration uses a common Latin letters/figures subset, rather than every control function or national variant.<Cite ids={['ita2']} /></p>
          </section>

          <section className={s.section} aria-labelledby="email">
            <SectionTitle number="05" id="email" title="Email grows from the mailbox outward." deck="First a file on a shared machine. Then a network. Then a common language." />
            <div className={s.prose}>
              <h3>Mail before the network</h3><p>MIT’s CTSS MAIL, implemented by Tom Van Vleck and Noel Morris in 1965, lets users leave messages in a recipient’s private mailbox file. The recipient sees a notice at login. Van Vleck’s account describes addressing, recipient lists and protection against other users reading the mailbox. The essential separation is already there: the writer and reader need not be present together.<Cite ids={['ctss']} /></p>
              <h3>The @ separates a person from a host</h3><p>In 1971, Ray Tomlinson adapts local messaging to send mail between computers over ARPANET. The address names a user <em>at</em> a host. His work is a landmark in network mail, rather than the beginning of all computer messaging. The Computer History Museum describes mail’s rapid spread as networks grow in the early 1970s.<Cite ids={['tomlinson', 'chm']} /></p>
              <h3>Open rules make independent systems work together</h3><p>SMTP is published in RFC 821 in 1982. The lasting idea is that two independent systems can conduct a defined conversation to transfer a message. An email is stored and forwarded through servers, with queues and retries when a destination is temporarily unavailable. It does not require a dedicated live connection between the sender’s keyboard and the reader’s screen.<Cite ids={['smtp1982', 'smtp']} /></p>
              <h3>The envelope and the letter are different</h3><p>Internet mail separates transport addresses from the visible message. Headers such as From, To, Subject, Date and Message-ID describe the message. SMTP’s envelope tells the transport where to deliver it. Reply links and threading can use identifiers such as In-Reply-To and References. This is why what appears in a header is not, by itself, proof of a sender’s identity.<Cite ids={['format', 'smtp']} /></p>
              <h3>Attachments and international text</h3><p>MIME gives message bodies structure: content types, transfer encodings and multiple parts. A photograph, a document and alternative plain-text/HTML bodies can all live inside a message. Later internationalized-email specifications extend SMTP to support UTF-8 addresses; support still depends on the systems involved. These are separate developments from replacing telex’s five-bit alphabet with richer text encodings.<Cite ids={['mime', 'utf8']} /></p>
              <h3>The inbox becomes a place you can reach anywhere</h3><p>POP3 offers a straightforward way to retrieve mail from a server. IMAP manages server-side mailboxes, flags and searching, and lets an offline client resynchronize. Webmail moves the user interface into the browser: Microsoft’s 1997 Hotmail announcement emphasizes independence from an internet provider and access from a connected PC. Gmail’s 2004 announcement makes large storage and search central to the product.<Cite ids={['pop', 'imap', 'hotmail', 'gmail']} /></p>
            </div>
            <div className={s.protocols}><span className={s.eyebrow}>FOUR JOBS, FOUR LAYERS</span><dl><div><dt>Address & route</dt><dd>Domains and DNS locate the destination.</dd></div><div><dt>Transfer</dt><dd>SMTP hands messages between systems.</dd></div><div><dt>Describe content</dt><dd>Message headers and MIME describe what was sent.</dd></div><div><dt>Access & organize</dt><dd>IMAP, POP3 and service interfaces expose the mailbox.</dd></div></dl><Cite ids={['architecture']} /></div>
          </section>

          <section className={s.section} aria-labelledby="relatives">
            <SectionTitle number="06" id="relatives" title="The wider family of written connections." deck="Shared problems, different answers. Some are cousins; some only look alike." />
            <div className={s.relatives}>{relatives.map((item) => <div key={item.name}><div><span className={s.eyebrow}>{item.kind}</span><h3>{item.name}</h3></div><p>{item.text}<Cite ids={item.sources} /></p></div>)}</div>
            <div className={s.tableWrap} tabIndex={0} role="region" aria-label="Comparison of telex, email, fax and chat; scroll horizontally if needed"><table className={s.comparison}><caption>What actually travels?</caption><thead><tr><th scope="col">System</th><th scope="col">Destination</th><th scope="col">Message</th><th scope="col">Receiving experience</th></tr></thead><tbody><tr><th scope="row">Classic telex</th><td>Subscriber terminal</td><td>Coded characters</td><td>Print during the connection</td></tr><tr><th scope="row">Internet email</th><td>User / domain</td><td>Headers, body, MIME parts</td><td>Deliver to a mailbox; read later</td></tr><tr><th scope="row">Fax</th><td>Fax endpoint</td><td>Page image</td><td>Reproduce the document</td></tr><tr><th scope="row">SMS</th><td>Mobile number</td><td>Short text message</td><td>Message reaches a handset</td></tr><tr><th scope="row">Chat</th><td>User, room or channel</td><td>Messages / events</td><td>Conversation in a service or network</td></tr></tbody></table></div>
            <p className={s.caption}>A comparison of common use, not a universal feature matrix. Gateways, later terminals and individual services can change these boundaries.</p>
          </section>

          <section className={s.section} aria-labelledby="today">
            <SectionTitle number="07" id="today" title="The machines recede. The questions remain." deck="A present-day view, checked in October 2026." />
            <div className={s.prose}><p>The old public telex world has largely been shut down, but working machines remain. The i-Telex project connects teleprinters over the internet and recreates familiar operation, with interfaces for historic equipment. Amateur radio keeps RTTY in use as a text mode. Preservation here includes the act of communicating, as well as keeping an object on a shelf.<Cite ids={['itelex', 'rtty']} /></p><p>Email still illustrates a useful separation: the protocol, the provider and the client are different things. Common standards allow different organizations to exchange messages and different clients to access mailboxes. The desktop client, webmail and mobile app are ways to work with the correspondence, rather than separate inventions of email itself.<Cite ids={['architecture', 'imap']} /></p></div>
            <dl className={s.todayQuestions}>
              <div><dt>Who is really sending?</dt><dd>SPF checks whether a host is authorized for an envelope domain. DKIM adds a domain signature over selected message content. DMARC builds on these results and their alignment with the visible From domain. None establishes that a particular human is honest.<Cite ids={['spf', 'dkim', 'dmarc']} /></dd></div>
              <div><dt>Who can see the message?</dt><dd>TLS can protect a transport connection. That is a different boundary from encrypting content end to end for the intended reader. A secure connection to a mail service does not by itself hide the message from that service.<Cite ids={['tls']} /></dd></div>
              <div><dt>What does “sent” mean?</dt><dd>Accepted by an outgoing server, delivered to a mailbox and read by a person are distinct events. A telex answerback identifies a connected machine; SMTP acceptance assigns responsibility to a server. Neither event is a human acknowledgment.<Cite ids={['f60', 'smtp']} /></dd></div>
              <div><dt>Who does the filing?</dt><dd>The growing mailbox makes sorting another part of communication. Sieve standardizes rules for processing email. Desktop clients can also apply their own local rules and search indexes. A record becomes useful when it can be found again.<Cite ids={['sieve']} /></dd></div>
            </dl>
            <p className={s.editorialNote}>The connection we draw is a design observation: addressing, identity, availability and records keep returning in new forms. Similarity of purpose does not establish direct technical descent.</p>
          </section>

          <section className={`${s.section} ${s.namesake}`} aria-labelledby="namesake">
            <SectionTitle number="08" id="namesake" title="Why we call this mail client Telex." />
            <div className={s.prose}><p>The appeal is practical: type, transmit, keep a useful record. Our Telex is a modern email client built around a keyboard, compact lists and mail stored on your own disk. It connects to email services with IMAP and sends with SMTP.</p><p>Saved searches make those records easy to revisit. Rules match headers and attachments, with previews before they change mail. Optional threads keep related messages together, and local link inspection explains suspicious destinations without visiting them. <Link href={pages.features.path}>Explore the current features</Link> or <Link href={pages.security.path}>inspect their security limits and evidence</Link>.</p><p>The name is a reference to the earlier network. The paper tape in this site’s footer uses ITA2, including the same letters and figures shifts shown above. The app’s work happens in modern mailboxes; the old machines supply a useful reminder to make the tool direct, readable and responsive.</p></div>
            <figure className={s.appShot}><Shot id="inbox" alt="The Telex email client with compact message rows, a folder list and a reading pane." sizes="(max-width: 900px) calc(100vw - 64px), 820px" /><figcaption>03 / TELEX TODAY · A desktop email client for Windows and Linux.</figcaption></figure>
            <div className={s.endActions}><Link href={pages.demo.path} className="btn btn-primary">Try the live demo <span aria-hidden="true">→</span></Link><Link href={pages.about.path} className={s.textLink}>The ideas behind the app ↗</Link></div>
          </section>

          <section className={s.section} aria-labelledby="glossary"><SectionTitle number="09" id="glossary" title="A small glossary" /><dl className={s.glossary}>{glossary.map(([term, definition]) => <div key={term}><dt>{term}</dt><dd>{definition}</dd></div>)}</dl></section>

          <section className={`${s.section} ${s.sources}`} aria-labelledby="sources">
            <SectionTitle number="10" id="sources" title="Sources & further reading" deck="Museum collections, accounts by the people who built the systems, contemporary announcements and original standards." />
            <p className={s.sourceNote}>References are linked beside the relevant passages. Dates mark selected developments, rather than a claim to identify the first instance of every idea. Standards listed here are foundational documents; some have later updates. The hero image and office transcript are illustrations, not historical evidence.</p>
            {sourceGroups.map((group) => <details className={s.sourceGroup} key={group}><summary>{group} <span>{historySources.filter((source) => source.group === group).length} references</span></summary><ol>{historySources.map((source, index) => source.group === group ? <li key={source.id} value={index + 1}><a href={source.url}>{source.title} ↗</a><span>{source.publisher}</span></li> : null)}</ol></details>)}
            <p className={s.colophon}>Written for Telex · Sources checked 11 October 2026 · <a href="#top">Back to the beginning ↑</a></p>
          </section>
        </div>
      </div>
    </article>
  );
}
