import { ProjectMetrics } from './projectMetrics';
import { assert, test, describe } from 'vitest';

const PROJECT_ID = 'test-project';
const METRIC_NAME = 'test-metric';

describe.concurrent('ProjectMetrics', () => {
	test('set max and min correctly', async () => {
		const metrics = new ProjectMetrics(PROJECT_ID);

		metrics.setMetric(METRIC_NAME, 0);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 0, minValue: 0, maxValue: 0 });

		metrics.setMetric(METRIC_NAME, 1);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 1, minValue: 0, maxValue: 1 });

		metrics.setMetric(METRIC_NAME, -1);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: -1, minValue: -1, maxValue: 1 });

		metrics.setMetric(METRIC_NAME, 2);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: -1, maxValue: 2 });

		metrics.setMetric(METRIC_NAME, -2);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: -2, minValue: -2, maxValue: 2 });
	});

	test('handle malformed input', async () => {
		const metrics = new ProjectMetrics(PROJECT_ID);
		metrics.setMetric(METRIC_NAME, 1);
		metrics.setMetric(METRIC_NAME, 2);

		// Expected initial condition.
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 2 });

		// @ts-expect-error since we are intentionally violating the type.
		metrics.setMetric(METRIC_NAME, null);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 2 });

		// Pass invalid arguments and verify the are ignored.
		// @ts-expect-error since we are intentionally violating the type.
		metrics.setMetric(METRIC_NAME, undefined);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 2 });

		metrics.setMetric(METRIC_NAME, Infinity);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 2 });

		metrics.setMetric(METRIC_NAME, -Infinity);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 2 });

		metrics.setMetric(METRIC_NAME, NaN);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 2 });

		// Set a new valid value and observe the change.
		metrics.setMetric(METRIC_NAME, 3);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 3, minValue: 1, maxValue: 3 });
	});

	test('load previously stored data', async () => {
		let metrics = new ProjectMetrics(PROJECT_ID);
		metrics.setMetric(METRIC_NAME, 1);
		metrics.setMetric(METRIC_NAME, 3);
		metrics.setMetric(METRIC_NAME, 2);
		metrics.saveToLocalStorage();

		metrics = new ProjectMetrics(PROJECT_ID);
		metrics.loadFromLocalStorage();
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 2, minValue: 1, maxValue: 3 });
		metrics.saveToLocalStorage();

		metrics = new ProjectMetrics(PROJECT_ID);
		metrics.loadFromLocalStorage();
		metrics.setMetric(METRIC_NAME, 4);
		assert.deepEqual(metrics.getReport()[METRIC_NAME], { value: 4, minValue: 1, maxValue: 4 });
	});
});
