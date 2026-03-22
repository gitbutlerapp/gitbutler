<script lang="ts">
	import EnsureAuthorInfo from "$components/projectSettings/EnsureAuthorInfo.svelte";
	import ErrorBoundary from "$components/shared/ErrorBoundary.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import ChromeErrorBoundary from "$components/views/ChromeErrorBoundary.svelte";
	import ChromeHeader from "$components/views/ChromeHeader.svelte";
	import ChromeSidebar from "$components/views/ChromeSidebar.svelte";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import type { Snippet } from "svelte";

	const {
		projectId,
		children: children2,
		sidebarDisabled = false,
	}: { projectId: string; children: Snippet; sidebarDisabled?: boolean } = $props();

	const projectService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectService.getProject(projectId));
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project, { projectId })}
		<div class="chrome" use:focusable={{ vertical: true, activate: true }}>
			<ChromeHeader {projectId} projectTitle={project.title} actionsDisabled={sidebarDisabled} />
			<div class="chrome-body" use:focusable>
				<EnsureAuthorInfo {projectId} />
				<ChromeSidebar {projectId} disabled={sidebarDisabled} />
				<div class="chrome-content">
					<ErrorBoundary>
						{@render children2()}
					</ErrorBoundary>
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
