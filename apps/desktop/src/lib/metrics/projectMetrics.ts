export type ProjectMetricsReport = {
	[key: string]: ProjectMetric | undefined;
};

type ProjectMetric = {
	value: number;
	minValue: number;
	maxValue: number;
};

/**
 * Tracks arbitrary metrics and keeps track of min/max values. Please note that
 * reporting these numbers to the back end is delegated to the MetricsReporter
 * component.
 */
export class ProjectMetrics {
	private metrics: { [key: string]: ProjectMetric | undefined } = {};

	constructor(readonly projectId?: string) {}

	setMetric(key: string, value: number) {
		// Guard against upstream bugs feeding bad values.
		if (typeof value !== 'number' || !Number.isFinite(value) || Number.isNaN(value)) {
			console.warn(`Ignoring ${key} metric, bad value: ${value}`);
			return;
		}
		const oldEntry = this.metrics[key];
		if (oldEntry) {
			const { maxValue, minValue } = oldEntry;
			this.metrics[key] = {
				value,
				maxValue: Math.max(value, maxValue),
				minValue: Math.min(value, minValue)
			};
		} else {
			this.metrics[key] = {
				value,
				maxValue: value,
				minValue: value
			};
		}
	}

	getReport(): ProjectMetricsReport {
		// Return a copy since we keep mutating the metrics object,
		// and a report is specific to a point in time.
		return { ...this.metrics };
	}

	resetMetric(key: string) {
		delete this.metrics[key];
	}
}
