export type ProjectMetricsReport = {
	[key: string]: ProjectMetric | undefined;
};

type ProjectMetric = {
	value: number;
	minValue: number;
	maxValue: number;
};

export class ProjectMetrics {
	private metrics: { [key: string]: ProjectMetric | undefined } = {};

	constructor(readonly projectId?: string) {}

	setMetric(key: string, value: number) {
		const oldvalue = this.metrics[key];

		const maxValue = Math.max(value, oldvalue?.maxValue || value);
		const minValue = Math.min(value, oldvalue?.minValue || value);
		this.metrics[key] = {
			value,
			maxValue,
			minValue
		};
	}

	getMetrics(): ProjectMetricsReport {
		return this.metrics;
	}

	resetMetric(key: string) {
		delete this.metrics[key];
	}
}
