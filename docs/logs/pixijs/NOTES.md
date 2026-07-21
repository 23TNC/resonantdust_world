# pixijs login — edge event log (clean, isolated)

User `ExpPixi`, `focus=100,50`, clean env (edge restarted at debug, spacetime data wiped, no stray tabs).

**BYTE-IDENTICAL to the webgl run:**
- 1 session: `session established player_id=1024 name=ExpPixi`.
- All 6 upstreams connect in ~8ms: players, event_shard, data_shard, **tile**, **thing** — no cold-shard timeout.
- Same 9 zones subscribed + seeded (100, 82, 114, 83, 116, 98, 84, 115, 99).

Confirms there is NO edge-level difference between the pixijs and webgl clients — both drive core, core drives
the edge identically. (An earlier capture was contaminated by the still-open webgl tab HMR-reconnecting during
the redeploy; redone cleanly here with the webgl dev server stopped + the tab parked on the login form.)
