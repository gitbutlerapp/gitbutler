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
 * Watcher events use one shared streaming fetch per renderer while any watcher subscriptions are
 * active. Preload copies each event through main to Vite, which writes it as a server-sent event;
 * Chromium then exposes the copies in the request's EventStream tab. The real watcher callback runs
 * immediately, so throttling or failure of the diagnostic stream never changes event delivery. A
 * single stream avoids consuming one HTTP/1.1 connection per subscription, and mirrored events are
 * dropped while the response is backpressured rather than allowing diagnostics to grow memory.
 *
 * The dedicated `ipc.localhost` origin is essential even though it reaches the same Vite server.
 * Held trace responses must not occupy the app origin's connection pool and block navigation. The
 * permanent watcher stream and its relay traffic use `ipc-watcher.localhost` as well, so they cannot
 * reduce the connection budget available to ordinary IPC traces. The completion cache handles the
 * inverse race, where IPC finishes before a queued trace reaches Vite. Preload aborts its gate after
 * 60 seconds so tracing cannot freeze the app; Vite retains state only slightly longer to cover
 * timer skew and clean up requests whose connection does not close cleanly.
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
export const ipcTraceWatcherEventChannel = "lite:ipc-trace-watcher-event";
export const ipcTraceHost = "ipc.localhost";
export const ipcTraceWatcherHost = "ipc-watcher.localhost";
const ipcTracePathPrefix = "/__ipc/";
const ipcTraceAcceptPrefix = "application/json; trace-id=";
const ipcTraceStreamAcceptPrefix = "text/event-stream; trace-id=";
export const ipcTraceCompletionPath = `${ipcTracePathPrefix}complete`;
export const ipcTraceWatcherEventPath = `${ipcTracePathPrefix}watcher-event`;
const ipcTraceWatcherEventsPath = `${ipcTracePathPrefix}watcher-events`;

interface IpcTraceCompletion {
	traceId: string;
	ok: boolean;
	body: string;
	durationMs: number;
}

interface IpcTraceWatcherEvent {
	streamId: string;
	type: string;
	body: string;
}

const isIpcTraceId = (value: string): boolean =>
	/^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(value);

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

export const isIpcTraceWatcherEvent = (value: unknown): value is IpcTraceWatcherEvent =>
	typeof value === "object" &&
	value !== null &&
	"streamId" in value &&
	typeof value.streamId === "string" &&
	isIpcTraceId(value.streamId) &&
	"type" in value &&
	typeof value.type === "string" &&
	value.type.length > 0 &&
	!/[\r\n]/.test(value.type) &&
	"body" in value &&
	typeof value.body === "string" &&
	!/[\r\n]/.test(value.body);

const ipcTraceIdFromAccept = (
	accept: string | undefined,
	prefix = ipcTraceAcceptPrefix,
): string | undefined => {
	if (accept === undefined || !accept.startsWith(prefix)) return undefined;

	const traceId = accept.slice(prefix.length);
	return isIpcTraceId(traceId) ? traceId : undefined;
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
	type WatcherEventStream = { response: ServerResponse };

	// Preload gives up after 60 seconds; retain server state just long enough to cover timer skew.
	const traceRetentionMs = 65_000;
	const pendingTraces = new Map<string, PendingTrace>();
	const watcherEventStreams = new Map<string, WatcherEventStream>();
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

				if (requestUrl.pathname === ipcTraceWatcherEventsPath) {
					if (request.method !== "GET") {
						response.statusCode = 405;
						response.end("Method not allowed");
						return;
					}

					const streamId = ipcTraceIdFromAccept(request.headers.accept, ipcTraceStreamAcceptPrefix);
					if (streamId === undefined) {
						response.statusCode = 400;
						response.end("Missing IPC watcher stream ID");
						return;
					}

					watcherEventStreams.get(streamId)?.response.end();
					watcherEventStreams.set(streamId, { response });
					response.statusCode = 200;
					response.setHeader("access-control-allow-origin", "*");
					response.setHeader("cache-control", "no-cache, no-store");
					response.setHeader("content-type", "text/event-stream");
					response.flushHeaders();

					response.on("close", () => {
						if (watcherEventStreams.get(streamId)?.response === response)
							watcherEventStreams.delete(streamId);
					});
					return;
				}

				if (requestUrl.pathname === ipcTraceWatcherEventPath) {
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
						response.end("Invalid IPC watcher event payload");
						return;
					}

					if (!isIpcTraceWatcherEvent(payload)) {
						response.statusCode = 400;
						response.end("Invalid IPC watcher event");
						return;
					}

					const stream = watcherEventStreams.get(payload.streamId);
					if (stream === undefined) {
						response.statusCode = 404;
						response.end("IPC watcher event stream not found");
						return;
					}
					if (stream.response.writableNeedDrain) {
						response.statusCode = 429;
						response.end("IPC watcher event stream is backpressured");
						return;
					}

					stream.response.write(`event: ${payload.type}\ndata: ${payload.body}\n\n`);

					response.statusCode = 204;
					response.end();
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
