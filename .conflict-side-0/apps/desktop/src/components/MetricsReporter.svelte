<script lang="ts" module>
	export const HOUR_MS = 60 * 60 * 1000;
	export const INTERVAL_MS = 24 * HOUR_MS;
	export const DELAY_MS = 30 * 1000;
</script>

<script lang="ts">
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { SettingsMetrics } from '$lib/metrics/settingsMetrics.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { onMount } from 'svelte';
	import type { ProjectMetrics } from '$lib/metrics/projectMetrics';

	type Props = {
		projectId: string;
		projectMetrics: ProjectMetrics;
	};
	const { projectId, projectMetrics }: Props = $props();

	const posthog = getContext(PostHogWrapper);
	const lastReportMs = persisted<number | undefined>(undefined, `lastMetricsTs-${projectId}`);
	const projectService = getContext(ProjectService);

	new SettingsMetrics(projectService, projectMetrics);

	// Any interval or timeout must be cleared on unmount.
	let intervalId: any;
	let timeoutId: any;

	function reportMetrics() {
		// So we know if we should run on next onMount.
		lastReportMs.set(Date.now());

		const report = projectMetrics.project(projectId);
		if (!report) return;

		// Capture only individual changes.
		for (let [name, metric] of Object.entries(report)) {
			posthog.capture(`metrics:${name}`, {
				project_id: projectId,
				...metric
			});
			// We don't want to report the same static value over and over
			// again, so after successfully reporting a metric we reset it.
			projectMetrics.resetMetric(projectId, name);
		}
	}

	function startInterval() {
		// Delay to avoid first report happening before services have had time
		// to update the project metrics.
		timeoutId = setTimeout(async () => {
			reportMetrics();
		}, DELAY_MS);
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
