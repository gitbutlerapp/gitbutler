<script lang="ts">
	import { ProjectMetrics, type ProjectMetricsReport } from './projectMetrics';
	import { persisted } from '$lib/persisted/persisted';
	import { getContext } from '$lib/utils/context';
	import posthog from 'posthog-js';
	import { onMount } from 'svelte';

	const projectMetrics = getContext(ProjectMetrics);
	const projectId = projectMetrics.getProjectId();

	// Storing the last known values so we don't report same metrics twice
	const lastReport = persisted<ProjectMetricsReport>({}, `projectMetrics-${projectId}`);
	const hourMs = 60 * 60 * 1000;

	let lastCapture: { [key: string]: number | undefined } = {};
	let intervalId: any;

	function sample() {
		const metrics = projectMetrics.getMetrics();
		const projectId = projectMetrics.getProjectId();

		if (!metrics) return;

		// Capture only individual changes.
		for (let [metric, value] of Object.entries(metrics)) {
			const lastCaptureMs = lastCapture[metric];
			if (
				// If no previously recorded value.
				!$lastReport[metric] ||
				// Or the value has changed.
				$lastReport[metric] !== value ||
				// Or 24h have passed since metric was last caprured
				(lastCaptureMs && lastCaptureMs - Date.now() > 24 * hourMs)
			) {
				posthog.capture(`metrics:${metric}`, { project_id: projectId, count: value });
				lastCapture[metric] = Date.now();
			}
		}
		lastReport.set(metrics);
	}

	onMount(() => {
		intervalId = setInterval(() => {
			sample();
		}, hourMs / 12);
		return () => {
			if (intervalId) clearInterval(intervalId);
		};
	});
</script>

/// fdaslkdfja
