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
	import DomainButton from './DomainButton.svelte';
	import IconBranch from '$lib/icons/IconBranch.svelte';
	import IconSettings from '$lib/icons/IconSettings.svelte';

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
	<div class="flex h-7 flex-shrink-0" data-tauri-drag-region>
		<!-- Top spacer & drag region -->
	</div>
	<div class="mx-4 mb-4 mt-1">
		<BaseBranchCard {project} {branchController} {baseBranchStore} />
	</div>
	<div class="mb-4">
		<DomainButton href={`/${project.id}/board`} icon={IconBranch}>Active branches</DomainButton>
		<DomainButton href={`/${project.id}/settings`} icon={IconSettings}>Settings</DomainButton>
	</div>
	<YourBranches {project} {branchController} {branchesWithContentStore} />
	{#if githubContext}
		<PullRequests {pullRequestsStore} projectId={project.id} />
	{:else}
		<RemoteBranches {remoteBranchStore} projectId={project.id}></RemoteBranches>
	{/if}
	<Footer {user} />
	<AppUpdater {update} />
</div>
