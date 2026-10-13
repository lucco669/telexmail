# Offline mail risk checks

The opened-message preview asks `MailStore::inspect_message` for the normal message plus
`risk::analyse`'s advisory report. Listing, searching, syncing and filtering do not run the
scanner. The score is computed locally when a message is opened, and the UI caches the last
30 opened messages. Nothing is uploaded, looked up in DNS, fetched, moved, deleted or marked
read by the risk checker. The existing reading policy still controls read flags.

Scores are **heuristic points out of 100**, capped at 99, not percentages or calibrated
probabilities. 70 or more shows “High scam risk”, 40–69 “Suspicious”, and 1–39 “Some warning
signs”. Zero means no implemented warning signs were found in the checked content, not safe.
Every contributing category has a visible reason and weight. The engine keeps only the
strongest signal in each category; duplicated links cannot raise the score. A deceptive-link
signal replaces a correlated offsite-account-action signal.

| Category | Points |
| --- | --- |
| Claimed brand with unrelated sender domain | 25 |
| Sender domain resembling a protected brand | 50 |
| Link domain resembling a protected brand | 50 |
| Domain placed before `@` in a link to a different site | 65 |
| URL label names a different site | 25 |
| Invisible/bidirectional characters in sender or link | 40 |
| Protected brand's account/payment button goes offsite | 30; 50 for direct credentials or Netflix's third-party payment requests |
| Different Reply-To domain, with other signs and sensitive request | 15 |
| Account/payment request, with other signs already present | 15; 20 when the sender claims a protected brand |

The initial protected-brand catalogue covers Spotify, Netflix, mBank and PKO Bank Polski.
It is intentionally small and reviewable, not an exhaustive database of genuine senders.
Domain membership requires a dot boundary. Registrable-domain comparisons use the embedded
Public Suffix List, including multi-part suffixes and private hosting tenants. International
domains are parsed using URL/IDNA rules; a limited set of common confusable characters and
one-character misspellings/transpositions help identify lookalikes. Short bank names/acronyms are
exact matches only. This is not the complete Unicode confusable algorithm.

Brand claims come from the sender's display name, not arbitrary mentions in the subject or
body. HTML blockquotes and plain-text `>` quotes do not contribute links. Ordinary brand
mentions, matching subdomains, unrelated newsletter links, IDNs, payment wording or Reply-To
changes alone do not earn a high score. Third-party tracking can legitimately produce a low
URL-label mismatch score. Microsoft Safe Links' `url` parameter and Google's `/url?q=` format
are inspected locally; no redirect is followed and those services are not blanket allowlists.

Raw MIME inputs above 2 MiB skip MIME parsing and use the stored text. Text scans are capped
at 128 KiB, HTML at 256 KiB per body, URLs at 4096 bytes and links at 512 per message. The
report explicitly says when the check was incomplete. Attachments, embedded messages,
remote images, final redirects, reputation, SPF, DKIM and DMARC are not verified. Authentication-
Results, Received and X-Spam fields are not trusted or used to suppress warning signs.
Sender-owned headers are not proof of identity: [RFC 8601](https://www.rfc-editor.org/rfc/rfc8601.html).

Source references for the starting catalogue:

- [Spotify's guidance](https://support.spotify.com/us/article/spotify-email-legit/)
- [Spotify's short-link domain](https://developer.spotify.com/documentation/embeds/tutorials/using-the-oembed-api)
- [Netflix's guidance](https://help.netflix.com/en/node/65674)
- [mBank's official login guidance](https://www.mbank.pl/bezpieczenstwo/bezpieczny-bank/)
- [PKO's guidance](https://www.pkobp.pl/aktualnosci/bezpieczenstwo/uwaga-na-falszywe-wiadomosci-podszywajace-sie-pod-bank-19-09)

Run `cargo test -p telex-core risk` for the scanner regressions, `cargo test -p telex-core
opened_message_risk` for the store boundary, and the ignored `risk::tests::timing` test to
measure a synthetic HTML message. These examples are not a representative labelled-mail
corpus and do not establish a false-positive rate or probability calibration.
