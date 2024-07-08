export type ProjectMetricsReport = {
	[key: string]: string | number | undefined;
};

export class ProjectMetrics {
	private projectId: string | undefined;
	private metrics: { [key: string]: string | number } = {};

	getProjectId() {
		return this.projectId;
	}

	setProjectId(projectId: string) {
		this.projectId = projectId;
		this.metrics = {};
	}

	setMetric(key: string, value: string | number) {
		this.metrics[key] = value;
	}

	getMetrics(): ProjectMetricsReport | undefined {
		if (!this.projectId) return;
		return this.metrics;
	}
}
