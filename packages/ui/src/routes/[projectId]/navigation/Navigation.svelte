<script lang="ts">
	import type { Branch, BaseBranch, RemoteBranch, CustomStore } from '$lib/vbranches/types';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User } from '$lib/backend/cloud';
	import RemoteBranches from '../RemoteBranches.svelte';
	import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';
	import PullRequests from '../PullRequests.svelte';
	import BaseBranchCard from './BaseBranchCard.svelte';
	import type { Project } from '$lib/backend/projects';
	import YourBranches from './YourBranches.svelte';
	import Footer from './Footer.svelte';
	import AppUpdater from './AppUpdater.svelte';
	import type { Loadable } from '@square/svelte-store';
	import type { Update } from '../../updater';

	export let branchesWithContentStore: CustomStore<Branch[] | undefined>;
	export let remoteBranchStore: CustomStore<RemoteBranch[] | undefined>;
	export let baseBranchStore: CustomStore<BaseBranch | undefined>;
	export let pullRequestsStore: CustomStore<PullRequest[] | undefined>;
	export let branchController: BranchController;
	export let project: Project;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let user: User | undefined;
	export let update: Loadable<Update>;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
</script>

<div
	class="bg-color-5 border-color-4 z-30 flex w-80 shrink-0 flex-col border-r"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
	role="menu"
	tabindex="0"
>
	<!-- Top spacer -->
	<div class="flex h-7 flex-shrink-0" data-tauri-drag-region></div>
	<!-- Base branch -->
	<BaseBranchCard {project} {branchController} {baseBranchStore} />
	<!-- Your branches -->
	<YourBranches {project} {branchController} {branchesWithContentStore} />
	<!-- Remote branches -->
	{#if githubContext}
		<PullRequests {pullRequestsStore} projectId={project.id} />
	{:else}
		<RemoteBranches {remoteBranchStore} projectId={project.id}></RemoteBranches>
	{/if}
	<!-- Bottom spacer -->
	<Footer {user} {project} />
	<AppUpdater {update} />
</div>
