<script lang="ts">
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User } from '$lib/backend/cloud';
	import RemoteBranches from './RemoteBranches.svelte';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import PullRequests from './PullRequests.svelte';
	import BaseBranchCard from './BaseBranchCard.svelte';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import YourBranches from './YourBranches.svelte';
	import Footer from './Footer.svelte';
	import AppUpdater from './AppUpdater.svelte';
	import type { Loadable } from '@square/svelte-store';
	import type { Update } from '../../updater';
	import DomainButton from './DomainButton.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import type { BaseBranchService, VirtualBranchService } from '$lib/vbranches/branchStoresCache';
	import type { RemoteBranchService } from '$lib/stores/remoteBranches';
	import ProjectSelector from './ProjectSelector.svelte';

	export let vbranchService: VirtualBranchService;
	export let remoteBranchService: RemoteBranchService;
	export let baseBranchService: BaseBranchService;
	export let branchController: BranchController;
	export let project: Project;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let user: User | undefined;
	export let update: Loadable<Update>;
	export let prService: PrService;
	export let projectService: ProjectService;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
</script>

<div
	class="z-30 flex w-80 shrink-0 flex-col border-r"
	style:background-color="var(--bg-surface)"
	style:border-color="var(--border-surface)"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
	role="menu"
	tabindex="0"
>
	<div class="flex h-8 flex-shrink-0" data-tauri-drag-region>
		<!-- Top spacer & drag region -->
	</div>
	<div class="relative mx-4 mb-4 mt-1">
		<ProjectSelector {project} {projectService} />
	</div>
	<div class="mx-4 mb-4 mt-1">
		<BaseBranchCard {project} {baseBranchService} {branchController} {prService} />
	</div>
	<div class="mb-4">
		<DomainButton href={`/${project.id}/board`} icon="branch">Applied branches</DomainButton>
	</div>
	<YourBranches {project} {branchController} {vbranchService} />
	{#if githubContext}
		<PullRequests {prService} {githubContext} projectId={project.id} />
	{:else}
		<RemoteBranches {remoteBranchService} projectId={project.id}></RemoteBranches>
	{/if}
	<Footer {user} projectId={project.id} />
	<AppUpdater {update} />
</div>
