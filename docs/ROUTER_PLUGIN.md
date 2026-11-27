# Router plugin (round 4D item 5)

A router plugin decides which brain answers each job: for example small, quick models for light jobs and big ones for heavy jobs. Groovy's own switching stays as the backup. **The slot is defined but not enabled yet:** the Settings row ("Router plugin", Brains page) is disabled and Groovy always uses its own order.

Code: `windows/src-tauri/src/brains/router.rs` (`Router`, `Candidate`, `DefaultRouter`, `PluginRouter`), called from `brains/switch.rs` (`ranked`).

## Transport

- The plugin is a program the user chooses in Settings (setting key `assistant.routerPlugin`, empty for now).
- Groovy starts it **on demand**, once per ranking, with no console window, and stops it straight after.
- Groovy writes **one JSON line** to its stdin and reads **one JSON line** from its stdout.
- **Timeout: 300 ms.** No answer in time, a crash, or an answer Groovy can't read: Groovy's own order is used.

## In: one line

```json
{"job":{"kind":"commands","weight":"light"},
 "need":{"tools":true,"vision":false,"streaming":false,"webSearch":false,"local":false,"tier":"small"},
 "candidates":[
  {"brainId":"claude-code","model":"","auth":{"kind":"account","signed_in":true},
   "caps":{"tools":true,"vision":false,"streaming":false,"webSearch":false,"local":false,"tier":"large"},
   "limitedUntil":null,"lastLatencyMs":null},
  {"brainId":"google","model":"","auth":{"kind":"apiKey","present":true},
   "caps":{"tools":true,"vision":true,"streaming":false,"webSearch":false,"local":false,"tier":"small"},
   "limitedUntil":null,"lastLatencyMs":null}
 ]}
```

(Shown over several lines here; the real one is a single line.)

- `job.kind`: `chat`, `writing`, `commands` or `vision`. `job.weight`: `light` or `heavy`.
- `need.tier`: `small` for a light job, `large` for a heavy one. A candidate's `caps.tier` says which kind of model it uses.
- `candidates` come in Groovy's own order (cloud and subscription first, this PC last, the user's order on top). Only brains that are set up are listed.
- `auth.kind`: `account` (a signed-in subscription), `apiKey`, or `none` (a server on this PC).
- `limitedUntil`: Unix seconds until a limit resets, when known; `lastLatencyMs`: the last answer's time, when known.

## Out: one line

```json
{"ranked":["google","claude-code"]}
```

- Brain ids, best first. Unknown ids are ignored.
- **Any candidate the plugin leaves out is appended in Groovy's own order**, so a bad plugin can never leave the user with no brain.
- An empty list, no `ranked`, or anything that isn't this shape: Groovy's own order.

## What the plugin never receives

- **Never the prompt**, the conversation, a screenshot or any file.
- **Never an API key, a token or a sign-in file**, nor where they are kept.
- Nothing about the user: no name, no settings other than the list above.

The plugin only chooses the order. Groovy itself sends the request, under its own approval rules, and still moves to the next brain on a limit (when "Switch when a limit is hit" is on), a network failure or a missing sign-in.
