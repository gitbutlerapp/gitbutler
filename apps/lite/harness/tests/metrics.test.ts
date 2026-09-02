import type { UserProfile } from "@gitbutler/but-sdk";
import { beforeEach, describe, expect, test, vi } from "vitest";

interface CapturedEvent {
	distinctId: string;
	event: string;
	properties: Record<string, unknown>;
}

const mocks = vi.hoisted(() => ({
	client: {
		capture: vi.fn<(event: CapturedEvent) => void>(),
		getFeatureFlagPayload: vi.fn(),
		shutdown: vi.fn(),
	},
	getAppSettings: vi.fn(),
	getUserProfileLocal: vi.fn(),
	updateTelemetryDistinctId: vi.fn(),
}));

vi.mock("@gitbutler/but-sdk", () => ({
	getAppSettings: mocks.getAppSettings,
	getUserProfileLocal: mocks.getUserProfileLocal,
	updateTelemetryDistinctId: mocks.updateTelemetryDistinctId,
}));

vi.mock("posthog-node", () => ({
	PostHog: vi.fn(function PostHog() {
		return mocks.client;
	}),
}));

const profile = (id: number): UserProfile => ({
	id,
	name: null,
	login: null,
	email: null,
	picture: "",
	githubUsername: null,
});

const initMetrics = async (failureLimit?: {
	bucketSize: number;
	refillIntervalSeconds: number;
}) => {
	mocks.client.getFeatureFlagPayload.mockResolvedValue(failureLimit);
	const metrics = await import("../../electron/src/metrics.ts");
	await metrics.initMetrics("1.2.3");
	return metrics;
};

describe("api command metrics", () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
		mocks.getAppSettings.mockResolvedValue({
			telemetry: { appMetricsEnabled: true, appDistinctId: "user_1" },
		});
		mocks.getUserProfileLocal.mockResolvedValue(null);
		mocks.updateTelemetryDistinctId.mockResolvedValue(undefined);
		mocks.client.shutdown.mockResolvedValue(undefined);
	});

	test("captures successful commands with their sampling rate", async () => {
		const metrics = await initMetrics();
		const handler = vi.fn().mockResolvedValue("result");

		await expect(metrics.withApiCommandCapture("commitCreate", handler)(null)).resolves.toBe(
			"result",
		);
		const captured = mocks.client.capture.mock.lastCall?.[0];
		expect(captured?.distinctId).toBe("user_1");
		expect(captured?.event).toBe("api_command");
		expect(captured?.properties).toMatchObject({
			command: "commitCreate",
			failure: false,
			samplingRate: 1,
		});
		expect(captured?.properties).not.toHaveProperty("occurrenceCount");
		await metrics.shutdownMetrics();
	});

	test("applies failure limits, rethrows errors, and flushes before resetting on login", async () => {
		const metrics = await initMetrics({ bucketSize: 1, refillIntervalSeconds: 10 });
		const error = new Error("failed");
		const wrapped = metrics.withApiCommandCapture(
			"treeChangeDiffs",
			vi.fn().mockRejectedValue(error),
		);

		await expect(wrapped(null)).rejects.toBe(error);
		expect(mocks.client.getFeatureFlagPayload).toHaveBeenCalledWith(
			"lite-api-command-failure-limit",
			"gitbutler-lite-failure-limit",
		);
		const capturedFailure = mocks.client.capture.mock.lastCall?.[0];
		expect(capturedFailure?.distinctId).toBe("user_1");
		expect(capturedFailure?.event).toBe("api_command");
		expect(capturedFailure?.properties).toMatchObject({
			command: "treeChangeDiffs",
			failure: true,
			occurrenceCount: 1,
			samplingRate: 1,
		});

		mocks.client.capture.mockClear();
		await expect(wrapped(null)).rejects.toBe(error);
		expect(mocks.client.capture).not.toHaveBeenCalled();

		await metrics.metricsOnLogin(profile(2));
		expect(mocks.client.capture).toHaveBeenCalledOnce();
		expect(mocks.client.capture.mock.lastCall?.[0]).toMatchObject({
			distinctId: "user_1",
			properties: {
				command: "treeChangeDiffs",
				failure: true,
				occurrenceCount: 1,
				samplingRate: 1,
			},
		});
		mocks.client.capture.mockClear();
		await expect(wrapped(null)).rejects.toBe(error);
		const capturedAfterLogin = mocks.client.capture.mock.lastCall?.[0];
		expect(capturedAfterLogin?.distinctId).toBe("user_2");
		expect(capturedAfterLogin?.properties.occurrenceCount).toBe(1);
		await metrics.shutdownMetrics();
	});

	test("flushes suppressed failure counts on shutdown", async () => {
		const metrics = await initMetrics({ bucketSize: 1, refillIntervalSeconds: 60 });
		const error = new Error("failed");
		const treeDiffs = metrics.withApiCommandCapture(
			"treeChangeDiffs",
			vi.fn().mockRejectedValue(error),
		);
		const fetchStatus = metrics.withApiCommandCapture(
			"workspaceFetchStatus",
			vi.fn().mockRejectedValue(error),
		);

		await expect(treeDiffs(null)).rejects.toBe(error);
		await expect(treeDiffs(null)).rejects.toBe(error);
		await expect(treeDiffs(null)).rejects.toBe(error);
		await expect(fetchStatus(null)).rejects.toBe(error);
		await expect(fetchStatus(null)).rejects.toBe(error);
		mocks.client.capture.mockClear();
		mocks.client.shutdown.mockImplementationOnce(() => {
			expect(mocks.client.capture).toHaveBeenCalledTimes(2);
			return Promise.resolve();
		});

		await metrics.shutdownMetrics();

		expect(mocks.client.capture.mock.calls.map(([event]) => event.properties)).toEqual([
			expect.objectContaining({
				command: "treeChangeDiffs",
				failure: true,
				occurrenceCount: 2,
				samplingRate: 1,
			}),
			expect.objectContaining({
				command: "workspaceFetchStatus",
				failure: true,
				occurrenceCount: 1,
				samplingRate: 1,
			}),
		]);
	});

	test("waits for all active commands before draining suppressed failures", async () => {
		const metrics = await initMetrics({ bucketSize: 1, refillIntervalSeconds: 60 });
		const error = new Error("failed");
		let failPending: (error: Error) => void = () => {};
		let finishPending: () => void = () => {};
		const handler = vi
			.fn()
			.mockRejectedValueOnce(error)
			.mockImplementationOnce(
				() =>
					new Promise((_, reject) => {
						failPending = reject;
					}),
			);
		const wrapped = metrics.withApiCommandCapture("treeChangeDiffs", handler);
		const otherWrapped = metrics.withApiCommandCapture(
			"commitCreate",
			() =>
				new Promise<void>((resolve) => {
					finishPending = resolve;
				}),
		);

		await expect(wrapped(null)).rejects.toBe(error);
		const pending = wrapped(null);
		const otherPending = otherWrapped(null);
		mocks.client.capture.mockClear();
		const shutdown = metrics.shutdownMetrics();

		expect(shutdown).not.toBeNull();
		expect(mocks.client.shutdown).not.toHaveBeenCalled();
		failPending(error);
		await expect(pending).rejects.toBe(error);
		expect(mocks.client.shutdown).not.toHaveBeenCalled();
		finishPending();
		await otherPending;
		await shutdown;

		expect(mocks.client.capture.mock.calls.map(([event]) => event.properties)).toEqual([
			expect.objectContaining({ command: "commitCreate", failure: false, samplingRate: 1 }),
			expect.objectContaining({
				command: "treeChangeDiffs",
				failure: true,
				occurrenceCount: 1,
				samplingRate: 1,
			}),
		]);
		expect(mocks.client.shutdown).toHaveBeenCalledOnce();
	});

	test("bounds the wait for an active command", async () => {
		vi.useFakeTimers();
		try {
			const metrics = await initMetrics();
			const error = new Error("failed");
			let failPending: (error: Error) => void = () => {};
			const wrapped = metrics.withApiCommandCapture(
				"treeChangeDiffs",
				() =>
					new Promise((_, reject) => {
						failPending = reject;
					}),
			);
			const pending = wrapped(null);

			const shutdown = metrics.shutdownMetrics();
			expect(mocks.client.shutdown).not.toHaveBeenCalled();

			await vi.advanceTimersByTimeAsync(1_000);
			await shutdown;
			expect(mocks.client.shutdown).toHaveBeenCalledWith(1_000);

			failPending(error);
			await expect(pending).rejects.toBe(error);
			expect(mocks.client.capture).not.toHaveBeenCalled();
		} finally {
			vi.useRealTimers();
		}
	});

	test("keeps an in-progress shutdown reentrant until flushing finishes", async () => {
		const metrics = await initMetrics();
		let finishShutdown: () => void = () => {};
		mocks.client.shutdown.mockReturnValueOnce(
			new Promise<void>((resolve) => {
				finishShutdown = resolve;
			}),
		);

		const firstShutdown = metrics.shutdownMetrics();
		const secondShutdown = metrics.shutdownMetrics();

		expect(firstShutdown).toBe(secondShutdown);
		expect(mocks.client.shutdown).toHaveBeenCalledOnce();
		finishShutdown();
		await firstShutdown;
		expect(metrics.shutdownMetrics()).toBeNull();
	});
});
