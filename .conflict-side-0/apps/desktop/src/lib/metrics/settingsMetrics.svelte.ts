import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { ProjectService } from '$lib/project/projectService';

export class SettingsMetrics {
	constructor(
		private readonly projectService: ProjectService,
		private readonly projectMetrics: ProjectMetrics
	) {
		const project = readableToReactive(this.projectService.project);

		$effect(() => {
			if (!project.current) return;
			if (project.current.api) {
				this.projectMetrics.setMetric(project.current.id, 'cloudFunctionalityEnabled', 1);
				this.projectMetrics.setMetric(
					project.current.id,
					'bulterReviewEnabled',
					project.current.api.reviews ? 1 : 0
				);
			} else {
				this.projectMetrics.setMetric(project.current.id, 'cloudFunctionalityEnabled', 0);
				this.projectMetrics.setMetric(project.current.id, 'bulterReviewEnabled', 0);
			}
		});
	}
}
