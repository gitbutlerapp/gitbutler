import MetricsReporter, {
	HOUR_MS,
	DELAY_MS,
	INTERVAL_MS
} from '$components/MetricsReporter.svelte';
import { EventContext } from '$lib/analytics/eventContext';
import { POSTHOG_WRAPPER, PostHogWrapper } from '$lib/analytics/posthog';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { getSettingsdServiceMock } from '$lib/testing/mockSettingsdService';
import { render } from '@testing-library/svelte';
import { assert, test, describe, vi, beforeEach, afterEach } from 'vitest';

const PROJECT_ID = 'test-project';
const METRIC_NAME = 'test-metric';

describe('MetricsReporter', () => {
	let projectMetrics: ProjectMetrics;
	let context: Map<any, any>;
	let posthog: PostHogWrapper;

	beforeEach(() => {
		vi.useFakeTimers();
		projectMetrics = new ProjectMetrics();
		const MockSettingsService = getSettingsdServiceMock();
		const settingsService = new MockSettingsService();
		const eventContext = new EventContext();
		posthog = new PostHogWrapper(settingsService, eventContext);

		context = new Map([[POSTHOG_WRAPPER._key, posthog as any]]);
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test('should report on interval', async () => {
		const posthogMock = vi.spyOn(posthog, 'capture').mock;

		projectMetrics.setMetric(PROJECT_ID, METRIC_NAME, 1);
		render(MetricsReporter, { props: { projectId: PROJECT_ID, projectMetrics }, context });

		// Verify nothing happens immediately.
		assert.equal(posthogMock.calls.length, 0);

		// Verify metric has been reported after initial delay.
		await vi.advanceTimersByTimeAsync(DELAY_MS);
		assert.equal(posthogMock.calls.length, 1);

		assert.equal(posthogMock.lastCall?.[0], 'metrics:' + METRIC_NAME);
		assert.deepEqual(posthogMock.lastCall?.[1], {
			project_id: PROJECT_ID,
			value: 1,
			minValue: 1,
			maxValue: 1
		});

		// Metrics are reset after they have been reported, so we should expect
		// that previous value does not influence next max/min.
		projectMetrics.setMetric(PROJECT_ID, METRIC_NAME, -1);
		projectMetrics.setMetric(PROJECT_ID, METRIC_NAME, 1);
		projectMetrics.setMetric(PROJECT_ID, METRIC_NAME, 0);

		// Stop just one millisecond short of the reporting interval, and verify
		// it has not run again.
		await vi.advanceTimersByTimeAsync(INTERVAL_MS - DELAY_MS - 1);
		assert.equal(posthogMock.calls.length, 1);

		// Advance one millisecond and verify newly reported metrics.
		await vi.advanceTimersByTimeAsync(1);
		assert.equal(posthogMock.calls.length, 2);
		assert.deepEqual(posthogMock.lastCall?.[1], {
			project_id: PROJECT_ID,
			value: 0,
			minValue: -1,
			maxValue: 1
		});
	});

	test('run based on last timestamp', async () => {
		const captureMock = vi.spyOn(posthog, 'capture').mock;

		// System time set to 0 plus a full report interval.
		vi.setSystemTime(INTERVAL_MS);
		// Simulate last report to have been sent at hour 1.
		localStorage.setItem('lastMetricsTs-fake-id', HOUR_MS.toString());

		projectMetrics.setMetric(PROJECT_ID, METRIC_NAME, 1);
		render(MetricsReporter, { props: { projectId: PROJECT_ID, projectMetrics }, context });

		// Verify it did not fire immediately.
		assert.equal(captureMock.calls.length, 0);

		// Advance one hour so that a full interval has elapsed.
		await vi.advanceTimersByTimeAsync(HOUR_MS);
		assert.equal(captureMock.calls.length, 1);

		// Set new metric value since last one should have been cleared.
		projectMetrics.setMetric(PROJECT_ID, METRIC_NAME, 1);

		// Advance by full interval and ensure it fires again.
		await vi.advanceTimersByTimeAsync(INTERVAL_MS);
		assert.equal(captureMock.calls.length, 2);
	});
});
