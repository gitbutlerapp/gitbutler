/**
 * @file Development-only IPC tracing for Chromium DevTools.
 *
 * Electron IPC does not pass through Chromium's renderer network stack, so this mirrors each call
 * with real development HTTP traffic. A row describes the renderer-to-main boundary only; work
 * below it may include local reads, retries, or multiple network requests.
 *
 * 1. Before invoking IPC, preload POSTs the serialized arguments to
 *    `http://ipc.localhost:<vite-port>/__ipc/<scope>/<method>`. A media-type parameter on `Accept`
 *    carries the trace ID without adding noise to the URL or triggering a CORS preflight.
 * 2. The Vite plugin holds the response open until main receives the IPC result from preload and
 *    POSTs it to `/__ipc/complete` on the normal Vite origin. Relaying completion through main keeps
 *    that request out of DevTools and outside renderer network throttling.
 * 3. Vite responds with the result and IPC duration, and preload consumes the complete body before
 *    releasing the real IPC result. `Server-Timing` preserves the raw IPC duration separately from
 *    the Network row's end-to-end duration, which includes any simulated network delay. The JSON
 *    response and explicit `Content-Length` keep DevTools' Response and Size views populated.
 *
 * The dedicated `ipc.localhost` origin is essential even though it reaches the same Vite server.
 * Held trace responses must not occupy the app origin's connection pool and block navigation. The
 * completion cache handles the inverse race, where IPC finishes before a queued trace reaches Vite.
 * Preload aborts its gate after 60 seconds so tracing cannot freeze the app; Vite retains state only
 * slightly longer to cover timer skew and clean up requests whose connection does not close cleanly.
 *
 * Consuming the response makes latency and bandwidth throttling part of the IPC boundary, which can
 * approximate slow local work. Fetch is required so DevTools retains the response body. Tracing
 * fails open after a bounded delay. Lite's typed IPC arguments and results are JSON-compatible;
 * thrown errors are normalized for display, large payloads are truncated, and sensitive
 * askpass/clipboard arguments are redacted.
 *
 * Some preload-side code remains in `preload.cts`: sandboxed Electron preloads use a restricted
 * CommonJS environment that cannot load arbitrary local modules without introducing a bundling
 * step. Keep its duplicated trace constants in sync with this module.
 */

import { Buffer } from "node:buffer";
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Plugin } from "vite";

export const ipcTraceCompleteChannel = "lite:ipc-trace-complete";
export const ipcTraceHost = "ipc.localhost";
const ipcTracePathPrefix = "/__ipc/";
const ipcTraceAcceptPrefix = "application/json; trace-id=";
export const ipcTraceCompletionPath = `${ipcTracePathPrefix}complete`;

interface IpcTraceCompletion {
	traceId: string;
	ok: boolean;
	body: string;
	durationMs: number;
}

export const isIpcTraceCompletion = (value: unknown): value is IpcTraceCompletion =>
	typeof value === "object" &&
	value !== null &&
	"traceId" in value &&
	typeof value.traceId === "string" &&
	"ok" in value &&
	typeof value.ok === "boolean" &&
	"body" in value &&
	typeof value.body === "string" &&
	"durationMs" in value &&
	typeof value.durationMs === "number" &&
	Number.isFinite(value.durationMs) &&
	value.durationMs >= 0;

const ipcTraceIdFromAccept = (accept: string | undefined): string | undefined => {
	if (accept === undefined || !accept.startsWith(ipcTraceAcceptPrefix)) return undefined;

	const traceId = accept.slice(ipcTraceAcceptPrefix.length);
	return /^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(traceId) ? traceId : undefined;
};

const readRequestBody = async (request: IncomingMessage): Promise<string> =>
	await new Promise((resolve, reject) => {
		const chunks: Array<Buffer> = [];
		let byteLength = 0;
		let tooLarge = false;

		request.on("data", (chunk: Buffer | string) => {
			const buffer = typeof chunk === "string" ? Buffer.from(chunk) : chunk;
			byteLength += buffer.byteLength;
			if (byteLength > 5_000_000) tooLarge = true;
			else chunks.push(buffer);
		});
		request.on("end", () => {
			if (tooLarge) reject(new Error("IPC trace payload is too large"));
			else resolve(Buffer.concat(chunks).toString("utf8"));
		});
		request.on("error", reject);
	});

/** @internal Only loaded by the development Vite configuration. */
export const ipcTracePlugin = (): Plugin => {
	type PendingTrace = {
		response: ServerResponse;
		timeout: ReturnType<typeof setTimeout>;
	};

	// Preload gives up after 60 seconds; retain server state just long enough to cover timer skew.
	const traceRetentionMs = 65_000;
	const pendingTraces = new Map<string, PendingTrace>();
	const storedCompletions = new Map<
		string,
		{ completion: IpcTraceCompletion; timeout: ReturnType<typeof setTimeout> }
	>();

	const respond = (response: ServerResponse, completion: IpcTraceCompletion): void => {
		response.statusCode = completion.ok ? 200 : 500;
		response.setHeader("cache-control", "no-store");
		response.setHeader("content-length", String(Buffer.byteLength(completion.body)));
		response.setHeader("content-type", "application/json");
		response.setHeader("access-control-allow-origin", "*");
		response.setHeader("server-timing", `ipc;dur=${completion.durationMs}`);
		response.setHeader("timing-allow-origin", "*");
		response.end(completion.body);
	};

	return {
		name: "gitbutler-ipc-trace",
		apply: "serve",
		configureServer(server) {
			// oxlint-disable-next-line typescript/no-misused-promises -- Connect does not model async middleware.
			server.middlewares.use(async (request, response, next) => {
				const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
				if (!requestUrl.pathname.startsWith(ipcTracePathPrefix)) {
					next();
					return;
				}

				if (requestUrl.pathname === ipcTraceCompletionPath) {
					if (request.method !== "POST") {
						response.statusCode = 405;
						response.end("Method not allowed");
						return;
					}

					let payload: unknown;
					try {
						payload = JSON.parse(await readRequestBody(request));
					} catch {
						response.statusCode = 400;
						response.end("Invalid IPC trace payload");
						return;
					}

					if (!isIpcTraceCompletion(payload)) {
						response.statusCode = 400;
						response.end("Invalid IPC trace completion");
						return;
					}

					const pending = pendingTraces.get(payload.traceId);
					if (pending !== undefined) {
						clearTimeout(pending.timeout);
						pendingTraces.delete(payload.traceId);
						respond(pending.response, payload);
					} else {
						const existing = storedCompletions.get(payload.traceId);
						if (existing !== undefined) clearTimeout(existing.timeout);
						const timeout = setTimeout(
							() => storedCompletions.delete(payload.traceId),
							traceRetentionMs,
						);
						storedCompletions.set(payload.traceId, { completion: payload, timeout });
					}

					response.statusCode = 204;
					response.end();
					return;
				}

				if (request.method !== "POST") {
					response.statusCode = 405;
					response.end("Method not allowed");
					return;
				}

				try {
					await readRequestBody(request);
				} catch {
					response.statusCode = 400;
					response.end("Invalid IPC trace request");
					return;
				}

				const traceId = ipcTraceIdFromAccept(request.headers.accept);
				if (traceId === undefined) {
					response.statusCode = 400;
					response.end("Missing IPC trace ID");
					return;
				}

				const stored = storedCompletions.get(traceId);
				if (stored !== undefined) {
					clearTimeout(stored.timeout);
					storedCompletions.delete(traceId);
					respond(response, stored.completion);
					return;
				}

				const timeout = setTimeout(() => {
					pendingTraces.delete(traceId);
					response.statusCode = 504;
					response.setHeader("timing-allow-origin", "*");
					response.end("IPC trace timed out");
				}, traceRetentionMs);
				pendingTraces.set(traceId, { response, timeout });

				response.on("close", () => {
					const pending = pendingTraces.get(traceId);
					if (pending?.response !== response) return;
					clearTimeout(pending.timeout);
					pendingTraces.delete(traceId);
				});
			});
		},
	};
};
