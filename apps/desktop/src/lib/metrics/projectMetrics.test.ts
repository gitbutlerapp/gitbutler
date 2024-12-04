import { ProjectMetrics } from './projectMetrics';
import { assert, test, describe } from 'vitest';

describe.concurrent('ProjectMetrics', () => {
	test('set max and min correctly', async () => {
		const metrics = new ProjectMetrics('fake-id');
		const metricLabel = 'test_metric';

		metrics.setMetric(metricLabel, 0);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 0, minValue: 0, maxValue: 0 });

		metrics.setMetric(metricLabel, 1);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 1, minValue: 0, maxValue: 1 });

		metrics.setMetric(metricLabel, -1);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: -1, minValue: -1, maxValue: 1 });

		metrics.setMetric(metricLabel, 2);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: -1, maxValue: 2 });

		metrics.setMetric(metricLabel, -2);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: -2, minValue: -2, maxValue: 2 });
	});

	test('handle malformed input', async () => {
		const metrics = new ProjectMetrics('fake-id');
		const metricLabel = 'test_metric';
		metrics.setMetric(metricLabel, 1);
		metrics.setMetric(metricLabel, 2);

		// Expected initial condition.
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: 1, maxValue: 2 });

		// @ts-expect-error since we are intentionally violating the type.
		metrics.setMetric(metricLabel, null);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: 1, maxValue: 2 });

		// @ts-expect-error since we are intentionally violating the type.
		metrics.setMetric(metricLabel, undefined);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: 1, maxValue: 2 });

		metrics.setMetric(metricLabel, Infinity);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: 1, maxValue: 2 });

		metrics.setMetric(metricLabel, -Infinity);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: 1, maxValue: 2 });

		metrics.setMetric(metricLabel, NaN);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 2, minValue: 1, maxValue: 2 });

		// Set a new valid value and observe the change.
		metrics.setMetric(metricLabel, 3);
		assert.deepEqual(metrics.getReport()[metricLabel], { value: 3, minValue: 1, maxValue: 3 });
	});
});
