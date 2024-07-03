<script lang="ts">
	import { ProjectMetrics, type ProjectMetricsReport } from './projectMetrics';
	import { Project } from '$lib/backend/projects';
	import { persisted } from '$lib/persisted/persisted';
	import { shallowEqual } from '$lib/utils/compare';
	import { getContext } from '$lib/utils/context';
	import posthog from 'posthog-js';
	import { onMount } from 'svelte';

	const projectMetrics = getContext(ProjectMetrics);
	const project = getContext(Project);

	const lastReport = persisted<ProjectMetricsReport | undefined>(
		undefined,
		'projectMetrics-' + project.id
	);
	const hourMs = 60 * 60 * 1000;

	let lastCapture: number | undefined;
	let intervalId: any;

	function sample() {
		const report = projectMetrics.getReport();
		if (!report) return;

		const changed = !shallowEqual($lastReport, report);
		const timeSinceCaptureMs = lastCapture ? lastCapture - Date.now() : undefined;

		if (changed || (timeSinceCaptureMs && timeSinceCaptureMs > 24 * hourMs)) {
			posthog.capture('metrics:branch_count', { ...report, changed });
			lastReport.set(report);
			lastCapture = Date.now();
		}
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
