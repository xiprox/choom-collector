# Record

The documents agents write for Cyberpunk Co-op Tools live here, in the kinds [writing.md](../.claude/rules/writing.md) describes.

- [areas/product/plan.md](areas/product/plan.md): the stages from an empty repository to v1, and where each one stands.
- [areas/product/01-aims.md](areas/product/01-aims.md): who the app is for, its two jobs, and what it won't do.
- [areas/product/02-v1-scope.md](areas/product/02-v1-scope.md): what each feature of v1 does, and which of CP77 Log Collector's behaviors it keeps, changes, or drops.
- [areas/architecture/01-spine.md](areas/architecture/01-spine.md): the crates, what runs on which thread, how state reaches the window, and where the app's files live.
- [areas/network/01-relay-and-sessions.md](areas/network/01-relay-and-sessions.md): the relay's endpoints and frames, codes, the first messages, and reconnecting.
- [areas/interface/decisions.md](areas/interface/decisions.md): what the window's design has settled, with the values it draws with.
- [areas/interface/log.md](areas/interface/log.md): the mockup rounds that settled the design, the gpui spike's measurements, and the window drawn in gpui.
- [areas/build/log.md](areas/build/log.md): what the release pipeline produced, and what's untested in it.
- [decisions/001-rust-app-of-its-own.md](decisions/001-rust-app-of-its-own.md): Cyberpunk Co-op Tools is a Rust app of its own, for Cyberpunk 2077 only.
- [decisions/002-gpui-through-gpui-kit.md](decisions/002-gpui-through-gpui-kit.md): The window is drawn with gpui, through Longbridge's gpui-kit.
- [decisions/003-relay-forked-from-fsc-editor.md](decisions/003-relay-forked-from-fsc-editor.md): Every connection goes through a relay the host runs, forked from fsc-editor's.
- [decisions/004-releases-with-release-please-and-velopack.md](decisions/004-releases-with-release-please-and-velopack.md): Releases come from release-please, packed with Velopack.
