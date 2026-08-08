# Port: Real XAL / MSA Login (enable featured & online server list)

**Status:** Phases 0–2 done · Phase 3 (browser UI) implemented (Rust `showUrl`
override) but not end-to-end verified (game hasn't reached the interactive
sign-in step in headless runs).

<!--newline-->

## Goal

Make the game load the **featured / online server list** the same way the working
C++ reference (`/home/andrew/Files/Code/mcpelauncher-manifest`) does: run the
game's real XAL sign-in, drive it through the Rust `msa-daemon-client` chain, and
let the game fetch its gathering / layout endpoints with a real identity token.

## Why the list is empty right now

In Minecraft Bedrock 26.33 the featured server list is **downstream of a signed-in
XBL identity**, not raw HTTP. The HTTP layer in this Rust port already works
(verified: discovery `200`, OIDC `200`, keys `200`, POST bodies sent). What breaks
is the sign-in gate:

1. `rust_bridge.rs:292 patch_xal_initialize_noop` force-patches `XalInitialize →
   return S_OK`, so the game's real XAL never creates global state.
2. **26.x has a completely different login API than the old mcpelauncher chain.**
   The game's Java `MainActivity` declares new token natives
   (`getAccessToken`, `getClientId`, `getProfileId`, `getProfileName`,
   `setLoginInformation`, `setRefreshToken`, `setSession`, `clearLoginInformation`)
   that **the game `.so` does not export** — the launcher must provide them. This
   Rust port was built against the manifest's older API
   (`nativeInitializeXboxLive`, `Interop.invokeMSA`), which **does not exist in
   the 26.x DEX**, so the whole old chain is dead.
3. There is **no `com/microsoft/xal/browser/WebView` class in 26.x at all**; the
   browser natives live on `com/microsoft/xal/browser/BrowserLaunchActivity`
   (`showUrl`, `urlOperationSucceeded`, …) and are **exported by the game**.
4. Result in the run log: game fetches OIDC config + keys, then loops on
   `session/nonce` 401 + `GetTitlePublicKey` 400 + `OneCollector` 401 forever —
   it is parked at the sign-in state, never fetching `/api/v1.0/config/public`,
   `/api/v2.0/layout/*`, gathering, or cdn requests.

### Key facts established (investigation)

- **XAL is statically linked into `libminecraftpe.so` in 26.33** — all `Xal*`
  / `Xbl*` functions are exported by the game itself (`XalInitialize`,
  `XalUserGetWebAccountTokenSilentlyAsync`, `XblInitialize`, …). There is no
  separate XAL `.so` to load or link.
- **`libmaesdk.so` is the MSA/MAE token+telemetry SDK** (`AuthTokensController`,
  `Java_com_microsoft_applications_events_*`), not XAL. It has 1 unresolved
  symbol (`deflateBound`); its `DT_INIT` is skipped by the linker — likely
  non-fatal for sign-in but should be watched.
- **The old mcpelauncher Interop natives are DEAD on 26.x.** The 26.33 (and
  1.26.20) DEX `com/microsoft/xbox/idp/interop/Interop` declares only
  `initializeInterop(Landroid/content/Context;)Z`,
  `deinitializeInterop()Z`, `notificiation_registration_callback(Ljava/lang/String;)V`.
  `invokeMSA`, `invokeAuthFlow`, `initCLL`, `logCLL`, `ticket_callback` do **not
  appear anywhere in the 26.x DEX**. Our `xbox_live.rs` registers the old names,
  so those registrations silently attach to nothing. The 26.33 DEX also has a
  separate `com/microsoft/xboxtcui/Interop` with `tcui_completed_callback(I)V`.
- **The game only invokes `Interop.getLocalStoragePath` at startup** (observed in
  run: `XboxInterop: getLocalStoragePath -> /home/andrew/.local/share/mcpelauncher/`).
  The new login natives (`initializeInterop`, `MainActivity.getAccessToken`, …)
  are looked up via `GetMethodID` at VM-init but **never called** in the current
  patched build — the stall happens before any login-path call.
- **`MainActivity.nativeInitializeXboxLive` / `nativeinitializeLibHttpClient` do
  not exist in 26.x** — the old manifest bootstrap calls were removed. The
  `jni_support.rs:281-282` registrations are stale.
- **`jni_resolve_symbol` (`rust_bridge.rs:1801`) prefers the host binary**
  (`dlsym(NULL, sym)`) before the game. So any `#[no_mangle] pub extern "C" fn
  Java_com_mojang_minecraftpe_MainActivity_*` we add in Rust is automatically
  found by the existing registration and registered on the Rust VM. This is the
  clean seam for implementing the missing natives.
- `XalInitialize` return code is checked by the game; real XAL was previously
  unsafe because stubbed libHttpClient made `HCInitialize` fail. **The Rust HTTP
  layer is now real** (last commit), so real XAL init is plausible again — this
  is the main risk to verify in Phase 1.
- **The C++ reference (`mcpelauncher-manifest`) targets ≤1.21.x** and uses the
  old `Interop.invokeMSA` chain. It is only a partial reference for 26.x: the
  daemon/token plumbing transfers, but the native call surface does not.

## Reference files (mcpelauncher-manifest — partial, targets ≤1.21.x)

> For 26.x only the daemon/token plumbing transfers; the native call surface is
> different (see Phase 0 findings). Keep these for the token + daemon pieces.

| Concern | File |
|---------|------|
| Token acquisition via daemon + scope | `mcpelauncher-client/src/xbox_live_helper.cpp`, `xbox_auth.rs` |
| Old Interop natives (invokeMSA etc.) — **dead on 26.x**, for reference only | `mcpelauncher-client/src/jni/xbox_live.cpp` (+ `.h`) |
| Browser UI for interactive login | `xal_webview.h/cpp` (`qt`/`cli` variants, `xal_webview_factory.cpp`) |
| Token-store natives (26.x `MainActivity`) — **no reference; derives from DEX** | `mcpelauncher-client/src/jni/main_activity.cpp` (old-style store) |
| Native registration list (what the game expects) | `mcpelauncher-client/src/jni/jni_support.cpp:133-170` |
| `msa-daemon-client` (client lib, already ported) | `msa-daemon-client/src/*` |

## Phase plan

### Phase 0 — Instrument & confirm the stall ✅ DONE

Findings (this is now the **26.x API reality** the later phases must target):

- **DEX-verified native sets** (26.33 `c3.dump`/`c6.dump`, 1.26.20 identical):
  - `MainActivity` login API (all `PUBLIC NATIVE`, **not exported by the game**):
    `getAccessToken()Ljava/lang/String;`, `getClientId()Ljava/lang/String;`,
    `getProfileId()Ljava/lang/String;`, `getProfileName()Ljava/lang/String;`,
    `setLoginInformation(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V`,
    `setRefreshToken(Ljava/lang/String;)V`, `setSession(Ljava/lang/String;)V`,
    `clearLoginInformation()V`, plus `nativeSetIntegrityTokenErrorMessage`.
  - `Interop` natives (DEX-verified, **not exported by the game**):
    `initializeInterop(Landroid/content/Context;)Z`,
    `deinitializeInterop()Z`,
    `notificiation_registration_callback(Ljava/lang/String;)V`.
  - `BrowserLaunchActivity` natives (**exported by the game**):
    `checkIsLoaded()V`, `urlOperationCanceled(JZLjava/lang/String;)V`,
    `urlOperationFailed(JZLjava/lang/String;)V`,
    `urlOperationSucceeded(JLjava/lang/String;ZLjava/lang/String;)V`,
    `nativeLogBatch(I[Lcom/microsoft/xal/logging/LogEntry;)V`, `Error(Ljava/lang/String;)V`.
  - **No `com/microsoft/xal/browser/WebView` class anywhere in the 26.x DEX.**
  - Old manifest natives `invokeMSA`/`invokeAuthFlow`/`initCLL`/`logCLL`/
    `nativeInitializeXboxLive`/`nativeinitializeLibHttpClient` **do not exist**
    in 26.x DEX (count = 0). All old-API registrations in `xbox_live.rs` /
    `jni_support.rs:281-316` are dead code for 26.x.
- **Runtime trace** (30s run, 65 registered MainActivity natives):
  - `Interop.getLocalStoragePath` **is called** (→ data dir), `readConfigFile`/`getLocale`
    never fire.
  - The new login natives are looked up via `GetMethodID` at VM-init (warnings
    before the probes were registered) but **never invoked** — the game is parked
    at the sign-in state gate (`session/nonce` 401 + `GetTitlePublicKey` 400 +
    `OneCollector` 401 retry loop) before any login-path call.
  - Probe natives were added to `main_activity.rs` for all 8 `MainActivity` login
    methods + Interop `initializeInterop`/`deinitializeInterop`/`notificiation...`
    (log `LoginProbe:`/`XboxInterop:`), all registered. No `LoginProbe` fired in
    the 30s window — confirms the game never reaches the login API while the
    S_OK patch is in place.
- **Exit achieved:** we know exactly which natives the game requires and that it
  currently stalls before calling them.

### Phase 1 — Enable real XAL (remove the S_OK patch)

- Remove `patch_xal_initialize_noop` and its call in
  `core_patches_install_impl` (`rust_bridge.rs:292-323`, `:353`).
- Let the game's static `XalInitialize` run. Watch for:
  - `HCInitialize` E_FAIL via our Rust libHttpClient → if it happens, implement
    the `HCInitialize`-dependent piece in the HTTP layer (the call-init path that
    stubs previously skipped).
  - SIGSEGV from `libmaesdk.so` init (unresolved `deflateBound`) → resolve it
    (implement `deflateBound` shim or provide `libz` symbol) if it blocks.
- **Re-run with the probes** and capture which natives now fire. Expect the
  `MainActivity` login API + `Interop.initializeInterop` to become live once XAL
  init succeeds.
- **Exit:** game logs show XAL initializing (not S_OK-patched), and the 26.x
  login natives fire in order.

### Phase 2 — Wire the token store natives (26.x login API)

The 26.x game reads login state from the **`MainActivity` natives** — the launcher
is the identity store. Implement them backed by the Rust token chain:

- `getAccessToken` / `getClientId` / `getProfileId` / `getProfileName` — return
  stored values (persist in the data dir, keyed the same way the game does).
- `setLoginInformation` / `setRefreshToken` / `setSession` / `clearLoginInformation` —
  accept values from the game (or from our token acquisition) and store them.
- Implement `Interop.initializeInterop` (return true) and
  `notificiation_registration_callback`.
- Drive token acquisition from the msa-daemon chain (`xbox_auth.rs`, verified in
  `daemon_e2e.rs`); see below for scope + daemon availability.
- **Exit:** clicking sign-in fills `setLoginInformation` and
  `getAccessToken` returns a real token to the game.

### Phase 3 — Interactive sign-in UI

- When XAL needs a browser it uses **`BrowserLaunchActivity.showUrl`** — a **Java
  static method in the DEX** (access `0x0009`, 9 params), **not** a game-exposed
  native (`nm` on `libminecraftpe.so` shows only `checkIsLoaded` /
  `urlOperation*`). So the launcher must **override** `showUrl` on the Rust VM.
  **Done (`crates/client/src/jni/xal_browser.rs`):**
  - Registered `showUrl:(JLandroid/content/Context;Ljava/lang/String;Ljava/lang/String;I[Ljava/lang/String;[Ljava/lang/String;Z)V`
    via `jnivm_register_natives` on `com/microsoft/xal/browser/BrowserLaunchActivity`.
  - Body reads `opId` (a1), `starturl` (a3), `endurl` (a4) from the 4 gp args the
    VM forwards; CLI mode prints the URL + opens `xdg-open`, then reads the final
    redirect line from stdin and calls the game-exported
    `Java_..._urlOperationSucceeded(opId, finalUrl, false, "webkit-noDefault::0::none")`
    (mirrors `xal_webview_cli.cpp` / `mcpelauncher-client/src/jni/webview.cpp`).
  - Caveat: only the first 4 gp args survive the VM's `jni_CallStaticVoidMethod`
    shim (call_static.rs:50-63) — enough for CLI mode.
- **Not end-to-end verified:** headless runs park at the `device.auth.xboxlive.com`
  device-auth 404-loop (below) and never reach the interactive sign-in step, so
  `showUrl` is registered but never invoked in CI runs. Manual/real-X11 sign-in
  click is the way to exercise it.
- Stale `class_stubs.rs:489-519` `WebView` registration: **keep** (log-only
  missing-symbol warning, harmless; removing risks a FindClass no-op if any
  legacy path touches it).
- **Exit:** clicking "Sign in" opens the browser flow, user signs in, and
  `urlOperationSucceeded` hands the final URL back into XAL.

> **Known device-auth blocker:** `POST device.auth.xboxlive.com/device/authenticate`
> returns **500** (`content-length:0`) with an **all-zero `Signature` header and
> empty body** (`HTTP-AUTH >>>` logs). XAL still initializes and the menu
> renders, but the game loops retrying every ~10s instead of firing the
> `MainActivity` login natives. The 26.x token natives (`getAccessToken`,
> `setLoginInformation`, …) are registered/probed but never invoked in any run so
> far. Likely the device-auth request needs a properly ECDSA-signed body (our
> `Ecdsa.sign` handles the Java-side crypto path in `jni_support.rs`), or XAL
> expects the request body filled before dispatch — this is the next thing to fix
> for the browser flow to be reachable.

### Phase 4 — Complete the login handshake

- Confirm the game-side flow: `Interop.initializeInterop` → XAL init →
  silent token request (`getAccessToken`) → `MainActivity.setSession` /
  `setLoginInformation` round-trip. The old `auth_flow_callback` chain is gone on
  26.x; verify what 26.x uses instead from the DEX (`tcui_completed_callback` on
  `com/microsoft/xboxtcui/Interop` is a candidate).
- Persist tokens so silent login works next boot (verify `xal/` cache key path
  noted in `AGENTS.md`).
- **Exit:** game shows a signed-in account (gamertag) on the main menu.

### Phase 5 — Featured server list loads

- Confirm the game now issues `/api/v1.0/config/public`, `/api/v2.0/layout/*`,
  gathering and cdn requests with the XBL bearer token, and the Servers tab
  populates.
- Fix any remaining HTTP issues surfaced by these new authenticated requests
  (headers, streaming body, redirects).
- **Exit:** featured server list renders in-game, matching the reference launcher.

### Phase 6 — Cleanup & docs

- Update `AGENTS.md` and `README.md` known-issues: remove the "XAL disabled /
  offline-only / Mooshroom" wording; document the working sign-in + server list.
- Remove now-dead stubs (old-API `xbox_live.rs` natives, stale `jni_support.rs`
  registrations, `class_stubs.rs` `WebView` no-ops).
- **Exit:** `cargo build -p client` clean, main menu + sign-in + server list work.

## Risks / open questions

1. **Real `XalInitialize` under our libHttpClient** — the original reason for the
   S_OK patch. Must be re-tested now that HTTP is real; may need `HCInitialize`
   or `HCHttpCallCreate` glue fixes.
2. **26.x token-store natives are all new** — `getAccessToken`/`setLoginInformation`
   etc. are not exported by the game; we must implement the whole store. Exact
   Java-side call order (who calls `setLoginInformation` vs `setSession`) needs a
   re-run once XAL init actually proceeds (Phase 1).
3. **`libmaesdk.so` init skip** (unresolved `deflateBound`) — may be required for
   MSA token caching; add shim if the login flow depends on it.
4. **msa-daemon availability** — not installed; a dependency of the token route.
   The 26.x `Interop.initializeInterop` may expect the daemon protocol directly;
   verify against the DEX-side call before wiring.
5. **Call-arity cap** — the Rust VM passes ≤4 gp-register args to Java; the
   XBL callback chain needs the direct-export bypass already used in the HTTP
   layer (`call_request_failed_direct`).
6. **The server list still requires a real MS account** — a guest/anonymous
   token path (legacy mcpelauncher mode) is an alternative but the reference
   route (this plan) is what actually loads the list for real accounts.
7. **Old-API dead code must be removed** (Phase 6): `xbox_live.rs` `invokeMSA`/
   `invokeAuthFlow`/`initCLL`/`logCLL`, the `WebView` stub in `class_stubs.rs`,
   and `jni_support.rs:281-316` stale registrations — they mislead the next
   implementer and the game will never call them.

## Depends on

- Already done: `msa-daemon-client`, `daemon-utils`, `simple-ipc` Rust crates
  (`PORT_MSA_DAEMON_CLIENT.md`, `PORT_SIMPLEIPC.md`, `PORT_DAEMON_UTILS.md`).
- Already done: real libHttpClient HTTP delivery (commit `65a8a6f`).
- External: `mcpelauncher-msa-daemon` binary + a Microsoft account.
