export type ProjectMetricsReport = {
	project_id?: string;
	[key: string]: string | number | undefined;
};

export class ProjectMetrics {
	metrics: { [key: string]: string | number } = {};
	private projectId: string | undefined;

	setProject(projectId: string) {
		this.projectId = projectId;
	}

	setMetric(key: string, value: string | number) {
		this.metrics[key] = value;
	}

	getReport(): ProjectMetricsReport | undefined {
		if (!this.projectId) return;
		if (Object.keys(this.metrics).length === 0) return;
		return { ...this.metrics, project_id: this.projectId };
	}
}
