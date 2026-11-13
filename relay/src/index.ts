/**
 * The relay: one Durable Object per session code, holding the host's socket and up to
 * MAX_TESTERS testers' sockets.
 *
 * Forked from fsc-editor's Remote Connect relay (record/decisions/003-relay-forked-from-fsc-editor.md).
 * It routes: it reads the routing bytes at the front of a binary frame, and never the message
 * after them, and keeps nothing a player sent. Each socket carries its role in its attachment, so
 * a Durable Object woken from hibernation rebuilds its list of sockets from the sockets
 * themselves, and an idle session costs nothing.
 */

import { DurableObject } from "cloudflare:workers"

import {
  CLOSE_INTENTIONAL,
  CLOSE_SESSION_ENDED,
  HOST_GRACE_MS,
  ID_BYTES,
  MAX_TESTERS,
  TO_EVERYONE,
  TO_ONE,
  bytesToId,
  idToBytes,
  parseCode,
  type CloseReason,
  type ControlFrame,
  type TesterId,
} from "./protocol.ts"

interface Env {
  SESSIONS: DurableObjectNamespace<Session>
  /** Absent under `wrangler dev`, where there is nothing to protect. */
  JOIN_LIMIT?: RateLimit
}

type Attachment = { role: "host" } | { role: "guest"; guestId: TesterId }

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket")
      return new Response("Expected a WebSocket upgrade", { status: 426 })

    const url = new URL(request.url)
    // The app picks its code, so both endpoints name one; its shape is checked here, before
    // any Durable Object exists, so a sweep of malformed codes never costs an instantiation.
    const code = parseCode(url.searchParams.get("code") ?? "")
    if (!code) return new Response("Malformed code", { status: 400 })

    if (url.pathname === "/host") return sessionFor(env, code).fetch(request)
    if (url.pathname === "/join") {
      if (env.JOIN_LIMIT) {
        const ip = request.headers.get("CF-Connecting-IP") ?? "unknown"
        const { success } = await env.JOIN_LIMIT.limit({ key: ip })
        if (!success) return new Response("Too many attempts", { status: 429 })
      }
      return sessionFor(env, code).fetch(request)
    }
    return new Response("Not found", { status: 404 })
  },
}

function sessionFor(env: Env, code: string): DurableObjectStub<Session> {
  return env.SESSIONS.get(env.SESSIONS.idFromName(code))
}

export class Session extends DurableObject<Env> {
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    const code = parseCode(url.searchParams.get("code") ?? "") ?? ""
    return url.pathname === "/host"
      ? this.acceptHost(code, url.searchParams.get("resume") ?? "")
      : this.acceptTester()
  }

  private async acceptHost(code: string, offered: string): Promise<Response> {
    if (this.socketsFor("host").length > 0) return new Response("Session in use", { status: 409 })

    // A session whose host dropped can be taken back, but only by that host: the code alone is
    // enough to join a session, and must not be enough to take one over and its testers.
    const token = await this.ctx.storage.get<string>("resume")
    if (token && token !== offered) return new Response("Not the host of this session", { status: 403 })

    const resume = token ?? crypto.randomUUID()
    await this.ctx.storage.put("resume", resume)
    // The host is back inside the grace window, so the session isn't ending after all.
    await this.ctx.storage.deleteAlarm()

    const { client, server } = this.pair()
    this.ctx.acceptWebSocket(server, ["host"])
    server.serializeAttachment({ role: "host" } satisfies Attachment)
    sendControl(server, { type: "hosting", code, resume })
    return upgraded(client)
  }

  private acceptTester(): Response {
    if (this.socketsFor("host").length === 0) return this.refuse("unknown-code")
    if (this.socketsFor("guest").length >= MAX_TESTERS) return this.refuse("session-full")

    const guestId = crypto.randomUUID()
    const { client, server } = this.pair()
    this.ctx.acceptWebSocket(server, ["guest", guestId])
    server.serializeAttachment({ role: "guest", guestId } satisfies Attachment)

    sendControl(server, { type: "joined", guestId })
    for (const host of this.socketsFor("host")) sendControl(host, { type: "guest-joined", guestId })
    return upgraded(client)
  }

  /**
   * Turns a refusal into a closed socket rather than an HTTP error, because a WebSocket client
   * can't read a status code but does get the close frame and its reason.
   */
  private refuse(reason: CloseReason): Response {
    const { client, server } = this.pair()
    server.accept()
    server.close(CLOSE_SESSION_ENDED, reason)
    return upgraded(client)
  }

  webSocketMessage(ws: WebSocket, raw: string | ArrayBuffer): void {
    // Players send data only as binary frames; text is the relay's own, one way.
    if (typeof raw === "string") return
    const sender = ws.deserializeAttachment() as Attachment | null
    if (!sender) return
    const frame = new Uint8Array(raw)

    if (sender.role === "guest") {
      // The tester's ID comes from the socket's own attachment and never off the wire, so a
      // tester can't pose as another.
      const out = new Uint8Array(ID_BYTES + frame.length)
      out.set(idToBytes(sender.guestId), 0)
      out.set(frame, ID_BYTES)
      for (const host of this.socketsFor("host")) sendBinary(host, out)
      return
    }

    if (frame[0] === TO_EVERYONE) {
      const message = frame.subarray(1)
      for (const tester of this.socketsFor("guest")) sendBinary(tester, message)
    } else if (frame[0] === TO_ONE && frame.length >= 1 + ID_BYTES) {
      const to = bytesToId(frame.subarray(1, 1 + ID_BYTES))
      const message = frame.subarray(1 + ID_BYTES)
      for (const tester of this.socketsFor(to)) sendBinary(tester, message)
    }
  }

  async webSocketClose(ws: WebSocket, code: number): Promise<void> {
    const leaving = ws.deserializeAttachment() as Attachment | null
    if (!leaving) return

    if (leaving.role === "guest") {
      for (const host of this.socketsFor("host")) sendControl(host, { type: "guest-left", guestId: leaving.guestId })
      return
    }

    // Stopping on purpose ends the session now; anything else might be a connection dropping
    // for a moment, so the session waits to see whether the host returns.
    if (code === CLOSE_INTENTIONAL) {
      await this.endSession("host-stopped")
      return
    }
    await this.ctx.storage.setAlarm(Date.now() + HOST_GRACE_MS)
  }

  async webSocketError(ws: WebSocket): Promise<void> {
    await this.webSocketClose(ws, 0)
  }

  /** The grace window ran out. */
  async alarm(): Promise<void> {
    if (this.socketsFor("host").length > 0) return
    await this.endSession("host-left")
  }

  private async endSession(reason: CloseReason): Promise<void> {
    for (const tester of this.socketsFor("guest")) {
      sendControl(tester, { type: "closed", reason })
      tester.close(CLOSE_SESSION_ENDED, reason)
    }
    // The resume token goes too, so the code is free for whichever host opens it next.
    await this.ctx.storage.deleteAll()
  }

  private socketsFor(tag: string): WebSocket[] {
    return this.ctx.getWebSockets(tag)
  }

  private pair(): { client: WebSocket; server: WebSocket } {
    const [client, server] = Object.values(new WebSocketPair())
    return { client, server }
  }
}

function sendControl(ws: WebSocket, frame: ControlFrame): void {
  try {
    ws.send(JSON.stringify(frame))
  } catch {
    // A socket that has already gone is normal during teardown, and mustn't stop the rest of
    // the list from being told.
  }
}

function sendBinary(ws: WebSocket, bytes: Uint8Array): void {
  try {
    ws.send(bytes)
  } catch {
    // As in sendControl.
  }
}

function upgraded(client: WebSocket): Response {
  return new Response(null, { status: 101, webSocket: client })
}
