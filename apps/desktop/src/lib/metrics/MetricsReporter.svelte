<script lang="ts" module>
	export const HOUR_MS = 60 * 60 * 1000;
	export const INTERVAL_MS = 24 * HOUR_MS;
</script>

<script lang="ts">
	import { ProjectMetrics, type ProjectMetricsReport } from './projectMetrics';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { onMount } from 'svelte';

	const { projectMetrics }: { projectMetrics: ProjectMetrics } = $props();

	const projectId = projectMetrics.projectId;
	const posthog = getContext(PostHogWrapper);

	// Storing the last known values so we don't report same metrics twice
	const lastReport = persisted<ProjectMetricsReport>({}, `projectMetrics-${projectId}`);
	const lastReportMs = persisted<number | undefined>(undefined, `lastMetricsTs-${projectId}`);

	// Any interval or timeout must be cleared on unmount.
	let intervalId: any;
	let timeoutId: any;

	function reportMetrics() {
		// So we know if we should run on next onMount.
		lastReportMs.set(Date.now());

		const report = projectMetrics.getReport();
		if (!report) return;

		// Capture only individual changes.
		for (let [name, metric] of Object.entries(report)) {
			posthog.capture(`metrics:${name}`, {
				project_id: projectId,
				...metric
			});
			projectMetrics.resetMetric(name);
		}
		lastReport.set(projectMetrics.getReport());
	}

	function startInterval() {
		reportMetrics();
		intervalId = setInterval(() => {
			reportMetrics();
		}, INTERVAL_MS);
	}

	function scheduleFirstReport() {
		const now = Date.now();
		const lastMs = $lastReportMs;

		if (!lastMs || now - lastMs > INTERVAL_MS) {
			// It's been a while, start immediately.
			startInterval();
		} else {
			// Wait until full interval has passed then start.
			const duration = lastMs - now + INTERVAL_MS;
			timeoutId = setTimeout(() => {
				startInterval();
			}, duration);
		}
	}

	onMount(() => {
		scheduleFirstReport();
		return () => {
			if (intervalId) clearInterval(intervalId);
			if (timeoutId) clearTimeout(timeoutId);
		};
	});
</script>
