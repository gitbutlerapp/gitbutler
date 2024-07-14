export type ProjectMetricsReport = {
	[key: string]: string | number | undefined;
};

export class ProjectMetrics {
	private metrics: { [key: string]: string | number } = {};

	constructor(readonly projectId?: string) {}

	setMetric(key: string, value: string | number) {
		this.metrics[key] = value;
	}

	getMetrics(): ProjectMetricsReport {
		return this.metrics;
	}
}
