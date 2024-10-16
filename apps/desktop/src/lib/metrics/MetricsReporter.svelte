<script lang="ts">
	import { ProjectMetrics, type ProjectMetricsReport } from './projectMetrics';
	import { persisted } from '@gitbutler/shared/persisted';
	import posthog from 'posthog-js';
	import { onMount } from 'svelte';

	const { projectMetrics }: { projectMetrics: ProjectMetrics } = $props();
	const projectId = projectMetrics.projectId;

	// Storing the last known values so we don't report same metrics twice
	const lastReport = persisted<ProjectMetricsReport>({}, `projectMetrics-${projectId}`);
	const hourMs = 60 * 60 * 1000;

	let lastCapture: { [key: string]: number | undefined } = {};
	let intervalId: any;

	function sample() {
		const metrics = projectMetrics.getMetrics();
		if (!metrics) return;

		// Capture only individual changes.
		for (let [name, metric] of Object.entries(metrics)) {
			const lastCaptureMs = lastCapture[name];
			if (
				// If no previously recorded value.
				!$lastReport[name] ||
				// Or the value has changed.
				$lastReport[name]?.value !== metric?.value ||
				// Or 24h have passed since metric was last caprured
				(lastCaptureMs && lastCaptureMs - Date.now() > 24 * hourMs)
			) {
				posthog.capture(`metrics:${name}`, {
					project_id: projectId,
					...metric
				});
				lastCapture[name] = Date.now();
				projectMetrics.resetMetric(name);
			}
		}
		lastReport.set(metrics);
	}

	onMount(() => {
		intervalId = setInterval(() => {
			sample();
		}, 4 * hourMs);
		return () => {
			if (intervalId) clearInterval(intervalId);
		};
	});
</script>
