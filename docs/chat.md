# Fastcloud direct messages

These are Fastcloud conversations stored in the existing backend SQLite database.
They do not import, read or send SoundCloud's own private messages. Both people
must use a client supporting chat, sign in, and mutually follow each other on
SoundCloud. The client activates chat on startup using its authenticated identity;
names and avatar URLs come from SoundCloud rather than client-supplied profiles.

## API

All routes use the existing `Authorization: OAuth …` relay session. Disabled
Fastcloud accounts cannot access chat. The server derives the sender from the
session and checks conversation membership on every history/read/archive/report
request, including requests with guessed IDs.

| Method / path | Behaviour |
| --- | --- |
| POST `/v1/chat/activate` `{}` | Register/update the current account's chat profile |
| GET `/v1/chat/contacts` | Registered, approved mutual follows; own blocked list |
| GET `/v1/chat/inbox` | Conversation summaries, unread count and current user ID |
| POST `/v1/chat/threads` `{peerId}` | Open or restore a direct conversation |
| GET `/v1/chat/threads/{id}/messages` | Last 50 messages; `before` or `after` paginates |
| POST `/v1/chat/threads/{id}/messages` | Send `{text, nonce, attachment}` |
| POST `/v1/chat/threads/{id}/read` `{through}` | Advance the reader's watermark within this thread |
| POST `/v1/chat/threads/{id}/archive` `{}` | Hide only the current user's conversation |
| POST `/v1/chat/block` `{peerId, blocked}` | Block/unblock delivery between these accounts |
| POST `/v1/chat/threads/{id}/report` `{reason}` | Report `spam`/`harassment`, block and archive |
| GET `/v1/admin/chat/reports` | Owner only: 20 latest reports with five recent messages each |

Opening a thread and sending a new message recheck both SoundCloud follow
directions without using cached membership. Contacts and read-only eligibility
checks cache public relationships for up to 60 seconds. A follow lookup failure
does not grant permission; existing conversation history remains readable.
Mutual membership, recipient approval and either-side blocks are checked again
under the database lock before committing delivery.

The sender generates a random 32-character hexadecimal nonce. Its uniqueness is
scoped to the authenticated sender. A committed retry returns the same message;
the stored request hash rejects reuse with different text, music or thread.
Retries still acknowledge the committed message if the users unfollowed or
blocked each other afterwards. They cannot create a second delivery.

## Music and links

An attachment is `null` or `{kind: "track" | "playlist", id: number}`. The server
resolves its public title, artwork, owner and permalink through SoundCloud;
clients cannot forge attachment metadata or submit arbitrary fetch URLs. Albums
use the playlist endpoint. Explicit private music attachments are rejected.
Pasted public SoundCloud track/playlist links are resolved into music cards when
possible. If preview resolution fails, the original text is still retained.
Profile mentions and SoundCloud links open inside the client when clicked.

The Share control opens the recipient selector and places music in the chosen
conversation's draft. Sending requires the user's Send action. The previous
copy-link/open-SoundCloud-messages flow remains an explicit fallback.

## Persistence, bounds and privacy

The migration is idempotent and is included in the deployment preflight. The
existing backup worker includes chat tables. No additional service or token is
needed. Authentication tokens are not stored in chat tables or returned with
messages. Chat text is not logged or included in portable settings backups.
Transport uses the configured backend HTTPS address; SQLite messages are not
end-to-end encrypted.

Archiving preserves both participants' history; opening the thread or a new
message restores it. There is no individual-message editing/deletion. A report
shares recent conversation context with the administrator, as stated in the
report confirmation. Other users and the ordinary admin user list cannot read
private conversations through the chat API.

Limits: 4000 characters per message, 32 KiB request bodies, 50 messages per page,
100 inbox summaries, 200 registered contact candidates, 30 new messages per
minute and 300 per hour per sender. Polling runs every three seconds while the
chat page is visible and every 15 seconds for the inbox badge on other pages.
Background-hidden windows pause polling. Read receipts require a visible,
focused window at the end of the received conversation.

Drafts are kept in memory per conversation during the current application
session, retained after a send failure, and cleared on actual sign-out or account
change. Received messages and read markers live on the server, ready for future
mobile clients. Local account caches and late request completions are isolated.

## Rollout and checks

Deploy the backend before releasing the chat client. Use the existing
`bash deploy_checked.sh -f compose.ip.yaml -f compose.vpn.yaml`; preserve the
SQLite volume and wait for `Backend health verified`. Then release the client
with matching manifests and bilingual release notes. Agent work does not publish
or deploy automatically.

Backend tests cover authentication, mutual following, pagination, non-member
access, retries, rate limits, blocking, reporting and archive restoration. Native
bridge tests verify fixed routes and removal of untrusted sender/URL fields.
Frontend tests cover deduplication, account draft isolation and safe links. The
browser preview uses synthetic messages and never sends to real accounts.
Production delivery between two installed clients still requires the rollout.
