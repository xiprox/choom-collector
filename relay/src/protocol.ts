/**
 * What the relay and the app agree on: the code alphabet, the limits, the close codes, the
 * control frames, and the routing byte at the front of a host's binary frame.
 * record/areas/network/01-relay-and-sessions.md describes each; coop-core's relay client holds
 * the same values, so change both together.
 */

/**
 * No I, L, O, U, 0, or 1, since codes are read aloud in voice chat and typed by players looking
 * at the game, and those are the characters that get misheard or mistyped.
 */
export const CODE_ALPHABET = "23456789ABCDEFGHJKMNPQRSTVWXYZ"
export const CODE_LENGTH = 6

/** One host and this many testers, which bounds what a message to everyone costs. */
export const MAX_TESTERS = 8

/** How long a session waits for its host after the host's socket drops. */
export const HOST_GRACE_MS = 30_000

/** The host closes with this when it stops on purpose, which ends the session at once. */
export const CLOSE_INTENTIONAL = 4001
/** Testers' sockets close with this when the session ends, with the reason as the close text. */
export const CLOSE_SESSION_ENDED = 4002

/** The first byte of a host's binary frame: the rest goes to every tester. */
export const TO_EVERYONE = 0
/** The first byte of a host's binary frame: 16 bytes of a tester's ID follow, then the message. */
export const TO_ONE = 1
export const ID_BYTES = 16

/** A tester's ID, a UUID, as its text. */
export type TesterId = string

export type CloseReason = "host-left" | "host-stopped" | "session-full" | "unknown-code"

/** The relay's own frames, sent as JSON text. Data travels as binary frames. */
export type ControlFrame =
  | { type: "hosting"; code: string; resume: string }
  | { type: "joined"; guestId: TesterId }
  | { type: "guest-joined"; guestId: TesterId }
  | { type: "guest-left"; guestId: TesterId }
  | { type: "closed"; reason: CloseReason }

/** Takes a code as players type it, with a hyphen or spaces, in either case; null unless valid. */
export function parseCode(input: string): string | null {
  const cleaned = input.toUpperCase().replace(/[^A-Z0-9]/g, "")
  if (cleaned.length !== CODE_LENGTH) return null
  if (![...cleaned].every((char) => CODE_ALPHABET.includes(char))) return null
  return cleaned
}

/** `"0f1e…"`-style UUID text to its 16 bytes. */
export function idToBytes(id: TesterId): Uint8Array {
  const hex = id.replace(/-/g, "")
  const bytes = new Uint8Array(ID_BYTES)
  for (let i = 0; i < ID_BYTES; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16)
  return bytes
}

/** 16 bytes back to UUID text, as `crypto.randomUUID()` writes it. */
export function bytesToId(bytes: Uint8Array): TesterId {
  const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("")
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}
