import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createHttpTransport } from "./http-transport.ts";

/** A WebSocket the test plays the server for: it accepts, pushes and drops connections. */
class FakeWebSocket extends EventTarget {
	static readonly OPEN = 1;
	static readonly CLOSED = 3;
	static instances: Array<FakeWebSocket> = [];

	readonly url: string;
	readonly sent: Array<unknown> = [];
	readyState = 0;

	constructor(url: URL | string) {
		super();
		this.url = String(url);
		FakeWebSocket.instances.push(this);
	}

	send(data: string) {
		this.sent.push(JSON.parse(data));
	}

	close() {
		this.drop();
	}

	accept() {
		this.readyState = FakeWebSocket.OPEN;
		this.dispatchEvent(new Event("open"));
	}

	push(message: unknown) {
		this.pushRaw(JSON.stringify(message));
	}

	pushRaw(data: string) {
		this.dispatchEvent(new MessageEvent("message", { data }));
	}

	drop() {
		if (this.readyState === FakeWebSocket.CLOSED) return;
		this.readyState = FakeWebSocket.CLOSED;
		this.dispatchEvent(new Event("close"));
	}
}

const demo = "watcher:root/laptop/demo";
const other = "watcher:root/laptop/other";

let ticketsIssued = 0;
let serverReachable = true;
let serverHasEvents = true;
const fetchMock = vi.fn(async () => {
	if (!serverReachable) throw new TypeError("Failed to fetch");
	if (!serverHasEvents) return { ok: false, status: 404, statusText: "Not Found" };
	ticketsIssued++;
	return {
		ok: true,
		json: async () => ({ type: "success", subject: { ticket: `ticket-${ticketsIssued}` } }),
	};
});

const activity = (projectId: string) => ({
	name: `project://${projectId}/git/activity`,
	payload: { type: "gitActivity", subject: { headSha: "" } },
});

const latestSocket = () => {
	const socket = FakeWebSocket.instances.at(-1);
	if (!socket) throw new Error("No socket was opened");
	return socket;
};
const settle = () => vi.advanceTimersByTimeAsync(0);

const listen = (transport: ReturnType<typeof createHttpTransport>, channel: string) => {
	const received: Array<unknown> = [];
	const stop = transport.subscribe(channel, (payload) => received.push(payload));
	return { received, stop };
};

beforeEach(() => {
	vi.useFakeTimers();
	vi.spyOn(Math, "random").mockReturnValue(1);
	vi.stubGlobal("WebSocket", FakeWebSocket);
	vi.stubGlobal("fetch", fetchMock);
	FakeWebSocket.instances = [];
	ticketsIssued = 0;
	serverReachable = true;
	serverHasEvents = true;
	fetchMock.mockClear();
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the event stream", () => {
	test("opens one socket per transport, on a fresh ticket rather than the token", async () => {
		const transport = createHttpTransport("http://server");
		listen(transport, demo);
		listen(transport, other);
		await settle();

		expect(fetchMock).toHaveBeenCalledOnce();
		expect(fetchMock).toHaveBeenCalledWith("http://server/events/ticket", expect.anything());
		expect(FakeWebSocket.instances.map((socket) => socket.url)).toEqual([
			"ws://server/events?ticket=ticket-1",
		]);
	});

	test("leaves channels other than the watcher's to the host", async () => {
		listen(createHttpTransport("http://server"), "updateStatusChange");
		await settle();

		expect(fetchMock).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toEqual([]);
	});

	test("subscribes once a channel is listened to, and unsubscribes when its last listener leaves", async () => {
		const transport = createHttpTransport("http://server");
		const first = listen(transport, demo);
		const second = listen(transport, demo);
		await settle();
		latestSocket().accept();

		expect(latestSocket().sent).toEqual([{ type: "subscribe", projectId: "root/laptop/demo" }]);

		first.stop();
		expect(latestSocket().sent).toHaveLength(1);

		second.stop();
		expect(latestSocket().sent.at(-1)).toEqual({
			type: "unsubscribe",
			projectId: "root/laptop/demo",
		});
		expect(latestSocket().readyState).toBe(FakeWebSocket.CLOSED);
	});

	test("hands each event to its own channel's listeners", async () => {
		const transport = createHttpTransport("http://server");
		const onDemo = listen(transport, demo);
		const onOther = listen(transport, other);
		await settle();
		latestSocket().accept();

		latestSocket().push({ channel: demo, payload: { payload: { type: "gitActivity" } } });

		expect(onDemo.received).toEqual([{ payload: { type: "gitActivity" } }]);
		expect(onOther.received).toEqual([]);
	});

	test("ignores a frame that isn't JSON and keeps delivering", async () => {
		const transport = createHttpTransport("http://server");
		const onDemo = listen(transport, demo);
		await settle();
		latestSocket().accept();

		latestSocket().pushRaw("not json");
		latestSocket().push({ channel: demo, payload: { payload: { type: "gitActivity" } } });

		expect(onDemo.received).toEqual([{ payload: { type: "gitActivity" } }]);
	});

	test("refreshes each subscription once the server says it's live", async () => {
		const transport = createHttpTransport("http://server");
		const onDemo = listen(transport, demo);
		await settle();
		latestSocket().accept();
		expect(onDemo.received).toEqual([]);

		latestSocket().push({ type: "subscribed", projectId: "root/laptop/demo" });
		expect(onDemo.received).toEqual([activity("root/laptop/demo")]);

		// A project added to a socket that's already open gets the same treatment.
		const onOther = listen(transport, other);
		expect(latestSocket().sent.at(-1)).toEqual({
			type: "subscribe",
			projectId: "root/laptop/other",
		});
		latestSocket().push({ type: "subscribed", projectId: "root/laptop/other" });
		expect(onOther.received).toEqual([activity("root/laptop/other")]);
		expect(onDemo.received).toHaveLength(1);
	});

	test("refreshes nothing for a rejected subscription", async () => {
		const transport = createHttpTransport("http://server");
		const onDemo = listen(transport, demo);
		await settle();
		latestSocket().accept();

		latestSocket().push({ type: "rejected", projectId: "root/laptop/demo" });

		expect(onDemo.received).toEqual([]);
	});

	test("after a drop, resubscribes and catches up once live again", async () => {
		const transport = createHttpTransport("http://server");
		const onDemo = listen(transport, demo);
		await settle();
		latestSocket().accept();
		latestSocket().push({ type: "subscribed", projectId: "root/laptop/demo" });

		latestSocket().drop();
		await vi.advanceTimersByTimeAsync(1_000);
		latestSocket().accept();

		expect(FakeWebSocket.instances).toHaveLength(2);
		expect(latestSocket().url).toBe("ws://server/events?ticket=ticket-2");
		expect(latestSocket().sent).toEqual([{ type: "subscribe", projectId: "root/laptop/demo" }]);
		expect(onDemo.received).toHaveLength(1);

		latestSocket().push({ type: "subscribed", projectId: "root/laptop/demo" });
		expect(onDemo.received).toEqual([activity("root/laptop/demo"), activity("root/laptop/demo")]);
	});

	test("abandons a socket that goes silent and reconnects", async () => {
		const transport = createHttpTransport("http://server");
		listen(transport, demo);
		await settle();
		const first = latestSocket();
		first.accept();

		await vi.advanceTimersByTimeAsync(59_000);
		first.push({ type: "heartbeat" });
		await vi.advanceTimersByTimeAsync(59_000);
		expect(fetchMock).toHaveBeenCalledOnce();

		await vi.advanceTimersByTimeAsync(1_000 + 1_000);
		expect(first.readyState).toBe(FakeWebSocket.CLOSED);
		expect(fetchMock).toHaveBeenCalledTimes(2);
		expect(FakeWebSocket.instances).toHaveLength(2);
	});

	test("backs off while the server is unreachable, and starts over once it's back", async () => {
		const transport = createHttpTransport("http://server");
		listen(transport, demo);
		await settle();
		latestSocket().accept();
		serverReachable = false;
		latestSocket().drop();

		await vi.advanceTimersByTimeAsync(1_000);
		expect(fetchMock).toHaveBeenCalledTimes(2);
		await vi.advanceTimersByTimeAsync(1_999);
		expect(fetchMock).toHaveBeenCalledTimes(2);
		await vi.advanceTimersByTimeAsync(1);
		expect(fetchMock).toHaveBeenCalledTimes(3);

		serverReachable = true;
		await vi.advanceTimersByTimeAsync(4_000);
		expect(fetchMock).toHaveBeenCalledTimes(4);
		latestSocket().accept();
		latestSocket().drop();

		await vi.advanceTimersByTimeAsync(1_000);
		expect(fetchMock).toHaveBeenCalledTimes(5);
	});

	test("stops for good on a server without the route, as a plain local but-server is", async () => {
		serverHasEvents = false;
		const transport = createHttpTransport("http://server");
		listen(transport, demo);
		await settle();
		expect(fetchMock).toHaveBeenCalledOnce();

		await vi.advanceTimersByTimeAsync(60_000);
		// Switching projects subscribes again; the server's answer still stands.
		listen(transport, other);
		await settle();

		expect(fetchMock).toHaveBeenCalledOnce();
		expect(FakeWebSocket.instances).toEqual([]);
	});
});
