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

	test("applies failure limits, rethrows errors, and resets buckets on login", async () => {
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
		await expect(wrapped(null)).rejects.toBe(error);
		const capturedAfterLogin = mocks.client.capture.mock.lastCall?.[0];
		expect(capturedAfterLogin?.distinctId).toBe("user_2");
		expect(capturedAfterLogin?.properties.occurrenceCount).toBe(1);
		await metrics.shutdownMetrics();
	});
});
