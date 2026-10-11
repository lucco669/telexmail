// Original editorial copy. References are museums, firsthand accounts and published standards.
// Keep current-status statements dated; the timeline distinguishes inventions from adoption.
export const historySources = [
  { id: 'chappe', title: 'The optical telegraph', publisher: 'Musée des Arts et Métiers', url: 'https://artsandculture.google.com/asset/the-optical-telegraph-claude-chappe/GgELTlCgklxe7w?hl=en', group: 'Before the inbox' },
  { id: 'wheatstone', title: 'Cooke and Wheatstone five-needle telegraph, 1837', publisher: 'Science Museum Group', url: 'https://collection.sciencemuseumgroup.org.uk/objects/co33374/cooke-and-wheatstone-5-needle-telegraph-1837-telegraph', group: 'Before the inbox' },
  { id: 'morse', title: 'Morse–Vail telegraph key and the 1844 demonstration', publisher: 'Smithsonian Institution', url: 'https://avpreservation.si.edu/spotlight/electricityandmagnetism', group: 'Before the inbox' },
  { id: 'cable', title: 'Information Age: the transatlantic cable', publisher: 'Science Museum', url: 'https://blog.sciencemuseum.org.uk/information-age-10-years-on/', group: 'Before the inbox' },
  { id: 'baudot', title: 'Baudot type-printing multiplex telegraph', publisher: 'Science Museum Group', url: 'https://collection.sciencemuseumgroup.org.uk/objects/co33386/baudot-type-printing-multiplex-telegraph-complete-with-keyboard-sender', group: 'Before the inbox' },
  { id: 'telex1932', title: 'Modern Methods in the Inland Telegraph Service (1932)', publisher: 'Nature, contemporary report', url: 'https://www.nature.com/articles/130806b0', group: 'Telex and paper tape' },
  { id: 'f60', title: 'F.60: Operational provisions for international telex', publisher: 'ITU-T', url: 'https://www.itu.int/rec/T-REC-F.60/en', group: 'Telex and paper tape' },
  { id: 'ita2', title: 'S.1: International Telegraph Alphabet No. 2', publisher: 'ITU-T', url: 'https://www.itu.int/rec/T-REC-S.1/en', group: 'Telex and paper tape' },
  { id: 's-series', title: 'Telegraph terminal equipment: speed, shifts and answerback', publisher: 'ITU-T S-series recommendations', url: 'https://www.itu.int/rec/T-REC-S/en', group: 'Telex and paper tape' },
  { id: 'ctss', title: 'The History of Electronic Mail', publisher: 'Tom Van Vleck, CTSS MAIL coauthor', url: 'https://www.multicians.org/thvv/mail-history.html', group: 'Email and networks' },
  { id: 'tomlinson', title: 'Ray Tomlinson sends network mail, 1971', publisher: 'Centre for Computing History', url: 'https://www.computinghistory.org.uk/det/6116/first-e-mail-sent-by-ray-tomlinson', group: 'Email and networks' },
  { id: 'chm', title: 'Networking & the Web: a historical timeline', publisher: 'Computer History Museum', url: 'https://www.computerhistory.org/timeline/networking-the-web/', group: 'Email and networks' },
  { id: 'smtp1982', title: 'RFC 821: Simple Mail Transfer Protocol (1982)', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc821', group: 'Email and networks' },
  { id: 'smtp', title: 'RFC 5321: SMTP transport and delivery', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc5321', group: 'Email and networks' },
  { id: 'format', title: 'RFC 5322: Internet Message Format', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc5322', group: 'Email and networks' },
  { id: 'mime', title: 'RFC 2045: MIME message bodies', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc2045', group: 'Email and networks' },
  { id: 'pop', title: 'RFC 1939: Post Office Protocol, version 3', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc1939', group: 'Email and networks' },
  { id: 'imap', title: 'RFC 9051: IMAP, version 4rev2', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc9051', group: 'Email and networks' },
  { id: 'hotmail', title: 'Microsoft acquires Hotmail (1997)', publisher: 'Microsoft, contemporary announcement', url: 'https://news.microsoft.com/source/1997/12/31/microsoft-acquires-hotmail/', group: 'Email and networks' },
  { id: 'gmail', title: 'Google announces Gmail (1 April 2004)', publisher: 'Google, contemporary announcement', url: 'https://googlepress.blogspot.com/2004/04/google-gets-message-launches-gmail.html', group: 'Email and networks' },
  { id: 'utf8', title: 'RFC 6531: Internationalized email addresses', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc6531', group: 'Email and networks' },
  { id: 'tls', title: 'RFC 3207: SMTP over TLS', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc3207', group: 'Messages today' },
  { id: 'spf', title: 'RFC 7208: Sender Policy Framework', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc7208', group: 'Messages today' },
  { id: 'dkim', title: 'RFC 6376: DomainKeys Identified Mail', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc6376', group: 'Messages today' },
  { id: 'dmarc', title: 'RFC 7489: DMARC', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc7489', group: 'Messages today' },
  { id: 'sieve', title: 'RFC 5228: Sieve email filtering', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc5228', group: 'Messages today' },
  { id: 'bbs', title: 'The first CBBS and the bulletin-board tradition', publisher: 'BBS Preservation Project', url: 'https://bbspreservation.org/history/', group: 'Related systems' },
  { id: 'nntp', title: 'RFC 3977: Network News Transfer Protocol', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc3977', group: 'Related systems' },
  { id: 'irc', title: 'RFC 1459: Internet Relay Chat', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc1459.html', group: 'Related systems' },
  { id: 'sms', title: 'The first SMS, 1992', publisher: 'Vodafone, operator account', url: 'https://www.vodafone.com/news/newsroom/technology/25-anniversary-text-message', group: 'Related systems' },
  { id: 'teletext', title: 'Teletext: Ceefax and Oracle', publisher: 'BBC Engineering, December 1978', url: 'https://downloads.bbc.co.uk/rd/pubs/archive/pdffiles/engineering/bbc_engineering_111.pdf', group: 'Related systems' },
  { id: 'fax', title: 'T.30: Procedures for document facsimile transmission', publisher: 'ITU-T', url: 'https://www.itu.int/rec/T-REC-T.30/en', group: 'Related systems' },
  { id: 'xmpp', title: 'RFC 7622: XMPP address format', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc7622', group: 'Related systems' },
  { id: 'itelex', title: 'What is i-Telex?', publisher: 'i-Telex project', url: 'https://www.i-telex.net/i-telex/', group: 'Messages today' },
  { id: 'itelex-basics', title: 'How a telex connection works', publisher: 'i-Telex project', url: 'https://www.i-telex.net/grundlagen/', group: 'Telex and paper tape' },
  { id: 'rtty', title: 'Digital data modes: radioteletype', publisher: 'ARRL', url: 'https://www.arrl.org/digital-data-modes', group: 'Messages today' },
  { id: 'architecture', title: 'RFC 5598: Internet Mail Architecture', publisher: 'RFC Editor', url: 'https://www.rfc-editor.org/rfc/rfc5598', group: 'Email and networks' },
  { id: 'rcs', title: 'Rich Communication Services', publisher: 'GSMA', url: 'https://www.gsma.com/solutions-and-impact/technologies/networks/rcs/', group: 'Related systems' },
  { id: 'signal', title: 'Signal’s end-to-end encryption', publisher: 'Signal Support', url: 'https://support.signal.org/hc/en-us/articles/360007320391-Is-it-private-Can-I-trust-it', group: 'Messages today' },
  { id: 'matrix', title: 'Matrix specification: clients, rooms and federation', publisher: 'Matrix.org', url: 'https://spec.matrix.org/latest/', group: 'Related systems' },
  { id: 'threads', title: 'Use threads to organize discussions', publisher: 'Slack Help', url: 'https://slack.com/help/articles/115000769927-Use-threads-to-organize-discussions', group: 'Related systems' },
] as const;

export type HistorySourceId = (typeof historySources)[number]['id'];
export type Milestone = {
  date: string; title: string; text: string;
  branch: 'wire' | 'network' | 'both'; source: HistorySourceId;
};

export const milestones: Milestone[] = [
  { date: '1790s', title: 'A message outruns its messenger', text: 'Chappe’s optical telegraph relays coded positions between visible towers. The Paris–Lille line opens in 1794.', branch: 'wire', source: 'chappe' },
  { date: '1837 / 1844', title: 'Writing travels by electricity', text: 'Cooke and Wheatstone’s needle telegraph and the Morse–Vail system make distance a problem for signals rather than couriers.', branch: 'wire', source: 'wheatstone' },
  { date: '1866', title: 'A lasting Atlantic connection', text: 'After earlier attempts, a successful transatlantic cable connects the continents and helps build a worldwide telegraph network.', branch: 'wire', source: 'cable' },
  { date: 'Late 1800s', title: 'The receiver becomes a printer', text: 'Printing telegraphs, including Baudot’s five-unit system, turn coded signals into visible characters.', branch: 'wire', source: 'baudot' },
  { date: '1930s', title: 'Teleprinters get an exchange', text: 'Telex lets subscribers connect to other subscribers. A contemporary British report describes the service in 1932.', branch: 'wire', source: 'telex1932' },
  { date: '1965', title: 'A mailbox on a shared computer', text: 'Tom Van Vleck and Noel Morris implement CTSS MAIL at MIT. Users leave messages in other users’ files on the same system.', branch: 'network', source: 'ctss' },
  { date: '1971', title: 'Mail crosses between computers', text: 'Ray Tomlinson sends network mail over ARPANET and uses @ to separate the user from the host.', branch: 'network', source: 'tomlinson' },
  { date: '1978', title: 'The bulletin board goes online', text: 'Ward Christensen and Randy Suess launch CBBS in Chicago. Dial-up communities develop their own public and private message spaces.', branch: 'network', source: 'bbs' },
  { date: '1982', title: 'A common language for sending mail', text: 'RFC 821 publishes SMTP. Open specifications let independent systems agree on how to transfer messages.', branch: 'network', source: 'smtp1982' },
  { date: '1990s', title: 'Email carries more than plain ASCII', text: 'MIME introduces a structured way to describe content types and encodings: international text, multipart messages and attachments.', branch: 'network', source: 'mime' },
  { date: '1992', title: 'A short message reaches a mobile', text: 'Neil Papworth sends the first SMS over Vodafone’s network, from a computer to a mobile phone.', branch: 'network', source: 'sms' },
  { date: '1997', title: 'The browser becomes a mail client', text: 'Microsoft’s Hotmail acquisition announcement describes an established webmail service accessible independently of a user’s internet provider.', branch: 'network', source: 'hotmail' },
  { date: '2004', title: 'Search becomes part of the inbox', text: 'Google announces Gmail with one gigabyte of storage and search at the center of the experience.', branch: 'network', source: 'gmail' },
  { date: '2010', title: 'Old machines find a new network', text: 'i-Telex succeeds the earlier TelexPhone project, connecting teleprinters through TCP/IP while recreating telex-style operation.', branch: 'both', source: 'itelex' },
  { date: '2021', title: 'Mail synchronization keeps evolving', text: 'IMAP4rev2 is published as RFC 9051, including remote folders, flags, searching and offline resynchronization.', branch: 'network', source: 'imap' },
];

export const chapters = [
  ['timeline', 'A timeline of messages'], ['before', 'Before telex'], ['telex', 'The telex connection'],
  ['tape', 'Five bits, a whole language'], ['email', 'How email grew'], ['relatives', 'The wider family'],
  ['today', 'What survives today'], ['namesake', 'Why we call it Telex'], ['glossary', 'A small glossary'], ['sources', 'Sources & further reading'],
] as const;

export const glossary = [
  ['Answerback', 'A telex terminal’s automatically transmitted identity, requested to check which machine is connected.'],
  ['Baud', 'Symbols per second. On a classic binary telex line, one symbol carries one bit; character framing still takes time.'],
  ['Circuit switching', 'Establishing a connection between endpoints for a session, as a classic telex call does.'],
  ['ITA2', 'International Telegraph Alphabet No. 2: five-bit combinations interpreted in letters or figures mode.'],
  ['Store and forward', 'Keeping a message at an intermediate system, then passing it on when the next destination is available.'],
  ['SMTP', 'Simple Mail Transfer Protocol: the conversation used to transfer internet email between systems.'],
  ['IMAP / POP3', 'Two ways to access a mailbox. IMAP manages server-side messages and folders; POP3 primarily retrieves messages.'],
  ['MIME', 'Conventions that describe message content, encodings and parts, including attachments.'],
  ['TTY', 'Short for teletype or teletypewriter. The name also survives in computing for terminal devices.'],
  ['Federation', 'Independent services communicating through agreed protocols, rather than every user sharing one operator.'],
] as const;
