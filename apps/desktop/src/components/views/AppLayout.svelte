<script lang="ts">
	import EnsureAuthorInfo from "$components/projectSettings/EnsureAuthorInfo.svelte";
import ErrorBoundary from "$components/shared/ErrorBoundary.svelte";
import ReduxResult from "$components/shared/ReduxResult.svelte";
import AppErrorFallback from "$components/views/AppErrorFallback.svelte";
import AppHeader from "$components/views/AppHeader.svelte";
import AppSidebar from "$components/views/AppSidebar.svelte";
import WindowsChrome from "$components/views/WindowsChrome.svelte";
import { BACKEND } from "$lib/backend";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
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
	const backend = inject(BACKEND);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = $derived(settingsService.appSettings);
	const useWindowsCustomChrome = $derived(
		backend.platformName === "windows" && !($settingsStore?.ui.useNativeTitleBar ?? false),
	);
</script>

<ReduxResult {projectId} result={projectQuery.result}>
	{#snippet children(project, { projectId })}
		<div class="chrome" use:focusable={{ vertical: true, activate: true }}>
			{#if useWindowsCustomChrome}
				<WindowsChrome
					{projectId}
					projectTitle={project.title}
					actionsDisabled={sidebarDisabled}
				/>
			{:else}
				<AppHeader {projectId} projectTitle={project.title} actionsDisabled={sidebarDisabled} />
			{/if}
			<div class="chrome-body" use:focusable>
				<EnsureAuthorInfo {projectId} />
				<AppSidebar {projectId} disabled={sidebarDisabled} />
				<div class="chrome-content">
					<ErrorBoundary>
						{@render children2()}
					</ErrorBoundary>
				</div>
			</div>
		</div>
	{/snippet}
	{#snippet error(e)}
		<AppErrorFallback {projectId} error={e} />
	{/snippet}
</ReduxResult>

<style>
	.chrome {
		display: flex;
		flex: 1;
		flex-direction: column;
		max-width: 100%;
		background-color: var(--bg-2);
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
