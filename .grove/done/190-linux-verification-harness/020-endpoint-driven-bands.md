# 020-endpoint-driven-bands

**Kind:** work

## Goal

Extend the `010` harness with the **macOS golden + the in-process host→golden
TCP forward**, and run the **endpoint-driven smoke band minus OCR** green from
inside the Ubuntu HUT: `agent` HTTP actions, `input *`, `screen capture`/`size`,
and `screen record`→mp4 (the `170` ffmpeg-next encoder's **runtime** proof).
`screen find-text` (OCR) is deferred to `030`. See the node BRIEF for the shared
design and endpoint seam.

## Build on `010`

`010` leaves: HUT lifecycle, the provisioning-channel seam, the band driver,
the aarch64 binary + ffmpeg-8 `.so` bundle staging. `020` adds the golden and
the forward, then a second band.

## The golden endpoint

- Bring up the macOS golden via the CLI subprocess (matches `live-vm-gate`):
  `testanyware vm start --platform macos --json` → returns `id`; the per-VM spec
  at `<XDG_STATE_HOME|~/.local/state>/testanyware/vms/<id>.json` carries the
  golden's `agent {host,port}` and `vnc {host,port,password}`. Read it on the
  host to learn `golden_ip:8648` (agent) and the VNC host/port/password. Reuse
  `live-vm-gate`'s readiness wait (`agent snapshot` until the Finder menu bar
  renders) before driving the guest.
- `vm stop <id>` on teardown (extend the `Drop` guard to cover both VMs).

## The in-process forward (the reusable machinery)

- A tokio TCP proxy task inside the harness: bind `0.0.0.0:AFWD` → splice to
  `golden_ip:8648`; bind `0.0.0.0:VFWD` → splice to `golden_ip:VNC`. Bidirectional
  `tokio::io::copy` per connection; a shutdown signal on teardown. Bind on
  `0.0.0.0` (not `127.0.0.1`) so the guest reaches it via host-gateway.
- **host-gateway discovery (in-guest):** the guest's default route is the host.
  Resolve it over the channel, e.g. `ip route show default | awk '{print $3}'`
  (record the exact form that works against tart's NAT). Pure-parse the output in
  a unit-tested helper.
- In-guest the CLI then targets `--agent <gw>:AFWD` and `--vnc <gw>:VFWD`
  (+ `TESTANYWARE_VNC_PASSWORD=<pw>` env), per `resolve.rs`. No spec files needed.

## Endpoint-driven band (assert `--json`)

- `agent health` / `agent snapshot` (+ a couple of the `010`-ported HTTP actions
  like `agent inspect`/`window-*`) → assert the agent responds through the
  forward.
- `input click`/`key`/`type` → assert success envelopes (landing correctness is
  already covered by the macOS `live-vm-gate`; here the point is the RFB client
  *runs on aarch64-linux* and reaches the forwarded endpoint).
- `screen capture --region …` and `screen size` → assert a PNG/dimensions come
  back (proves the cross-built RFB decoder runs).
- `screen record --duration 2 --fps 10 -o …` → assert a plausible MP4 with
  frames (magic `ftyp`, frames ≥ ~fps). **This is the runtime proof of the
  ffmpeg-8 `gpl-shared` libx264 encoder on aarch64-linux** — the thing `170`
  could only *link*. If the bundle is missing libx264, `ffmpeg.rs` errors "no
  libav encoder … is this ffmpeg built with libx264/libx265?" — confirm the
  BtbN `gpl-shared` bundle includes the codec `.so`s (it does; verify staged).

## Done when

- The harness, with the golden + forward wired, runs the endpoint-free **and**
  endpoint-driven (minus OCR) bands **green** in one `TESTANYWARE_LINUX_HARNESS=1`
  invocation, tearing down both VMs and the forward.
- `screen record` produces a real MP4 from inside the HUT (ffmpeg-8 runtime
  proven on aarch64-linux).
- The forward + host-gateway discovery are factored as the shared machinery the
  Windows harness reuses (only the provisioning channel differs).

## Notes

- Best-effort vs hard-fail: a flaky single agent action shouldn't mask the band;
  collect per-check results and assert once at the end (live-vm-gate pattern).
- Guest→host-gateway is the only reliable NAT edge (ADR-0009) — do **not** try to
  route the guest directly to the golden's IP.
