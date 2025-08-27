<script lang="ts">
	import ChromeErrorBoundary from '$components/ChromeErrorBoundary.svelte';
	import ChromeHeader from '$components/ChromeHeader.svelte';
	import ChromeSidebar from '$components/ChromeSidebar.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	const {
		projectId,
		children: children2,
		sidebarDisabled = false
	}: { projectId: string; children: Snippet; sidebarDisabled?: boolean } = $props();

	const projectService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectService.getProject(projectId));
</script>

<ReduxResult {projectId} result={projectResult.current}>
	{#snippet children(project, env)}
		<div class="chrome">
			<ChromeHeader
				projectId={env.projectId}
				projectTitle={project.title}
				actionsDisabled={sidebarDisabled}
			/>
			<div class="chrome-body">
				<ChromeSidebar projectId={env.projectId} disabled={sidebarDisabled} />
				<div class="chrome-content">
					{@render children2()}
				</div>
			</div>
		</div>
	{/snippet}
	{#snippet error(e)}
		<ChromeErrorBoundary {projectId} error={e} />
	{/snippet}
</ReduxResult>

<style>
	.chrome {
		display: flex;
		flex: 1;
		flex-direction: column;
		max-width: 100%;
		background-color: var(--clr-bg-2);
	}

	.chrome-body {
		display: flex;
		flex-grow: 1;
		height: 100%;
		overflow: hidden;
	}

	.chrome-content {
		display: flex;
		flex-grow: 1;
		align-items: self-start;
		padding: 0 14px 14px 0;
		overflow: hidden;
	}
</style>
