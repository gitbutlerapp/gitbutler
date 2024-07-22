export type ProjectMetricsReport = {
	[key: string]: string | number | undefined;
};

export class ProjectMetrics {
	private metrics: { [key: string]: number | undefined } = {};

	constructor(readonly projectId?: string) {}

	setMetric(key: string, value: number) {
		const oldvalue = this.metrics[key];
		this.metrics[key] = value;

		const maxKey = key + '-max';
		const minKey = key + '-min';
		this.metrics[maxKey] = Math.max(value, oldvalue || value);
		this.metrics[minKey] = Math.min(value, oldvalue || value);
	}

	getMetrics(): ProjectMetricsReport {
		return this.metrics;
	}

	resetMinMax(key: string) {
		const maxKey = key + '-max';
		const minKey = key + '-min';
		delete this.metrics[maxKey];
		delete this.metrics[minKey];
	}
}
