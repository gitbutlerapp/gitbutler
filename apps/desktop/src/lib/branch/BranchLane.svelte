<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import { Project } from '$lib/backend/projects';
	import { projectLaneCollapsed } from '$lib/config/config';
	import { stackingFeature } from '$lib/config/uiFeatureFlags';
	import FileCard from '$lib/file/FileCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { createGitHostChecksMonitorStore } from '$lib/gitHost/interface/gitHostChecksMonitor';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { createGitHostPrMonitorStore } from '$lib/gitHost/interface/gitHostPrMonitor';
	import { createGitHostPrServiceStore } from '$lib/gitHost/interface/gitHostPrService';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import Stack from '$lib/stack/Stack.svelte';
	import {
		createIntegratedCommitsContextStore,
		createLocalCommitsContextStore,
		createLocalAndRemoteCommitsContextStore,
		createRemoteCommitsContextStore
	} from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { RemoteFile, VirtualBranch } from '$lib/vbranches/types';
	import {
		getContext,
		getContextStoreBySymbol,
		createContextStore
	} from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import lscache from 'lscache';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	const { branch }: { branch: VirtualBranch } = $props();

	const gitHost = getGitHost();

	// BRANCH SERVICE
	const prService = createGitHostPrServiceStore(undefined);
	$effect(() => prService.set($gitHost?.prService()));

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getGitHostListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === branch.upstream?.givenName));
	const sourceBranch = $derived(listedPr?.sourceBranch);
	const prNumber = $derived(listedPr?.number);

	const gitHostPrMonitorStore = createGitHostPrMonitorStore(undefined);
	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	$effect(() => gitHostPrMonitorStore.set(prMonitor));

	const gitHostChecksMonitorStore = createGitHostChecksMonitorStore(undefined);
	const checksMonitor = $derived(sourceBranch ? $gitHost?.checksMonitor(sourceBranch) : undefined);
	$effect(() => gitHostChecksMonitorStore.set(checksMonitor));

	// BRANCH
	const branchStore = createContextStore(VirtualBranch, branch);
	const selectedOwnershipStore = createContextStore(
		SelectedOwnership,
		SelectedOwnership.fromBranch(branch)
	);
	const branchFiles = writable(branch.files);

	$effect(() => {
		branchStore.set(branch);
		selectedOwnershipStore.update((o) => o?.update(branch));
		branchFiles.set(branch.files);
	});

	// COMMITS
	const localCommits = createLocalCommitsContextStore([]);
	const localAndRemoteCommits = createLocalAndRemoteCommitsContextStore([]);
	const remoteCommits = createRemoteCommitsContextStore([]);
	const integratedCommits = createIntegratedCommitsContextStore([]);

	const allUpstreamCommits = $derived(branch.upstreamData?.commits ?? []);

	$effect(() => {
		localCommits.set(branch.localCommits);
		localAndRemoteCommits.set(branch.remoteCommits);
		remoteCommits.set(allUpstreamCommits.filter((c) => !c.relatedTo));
		integratedCommits.set(branch.integratedCommits);
	});

	const project = getContext(Project);
	const fileIdSelection = new FileIdSelection(project.id, branchFiles);
	const selectedFile = fileIdSelection.selectedFile;
	const commitId = $derived($selectedFile?.[0]);
	const selected = $derived($selectedFile?.[1]);
	setContext(FileIdSelection, fileIdSelection);

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let rsViewport: HTMLElement | undefined = $state();

	const commitBoxOpen = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);
	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + project.id);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number | undefined = $state(undefined);

	fileWidth = lscache.get(fileWidthKey + branch.id);

	let isLaneCollapsed = $state(projectLaneCollapsed(project.id, branch.id));
	$effect(() => {
		if ($isLaneCollapsed) {
			fileIdSelection.clear();
		}
	});
</script>

<div class="wrapper" data-tauri-drag-region>
	{#if $stackingFeature}
		<Stack {commitBoxOpen} {isLaneCollapsed} />
	{:else}
		<BranchCard {commitBoxOpen} {isLaneCollapsed} />
	{/if}

	{#if selected}
		<div
			class="file-preview"
			bind:this={rsViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={`${fileWidth || $defaultFileWidthRem}rem`}
		>
			<FileCard
				isUnapplied={false}
				conflicted={selected.conflicted}
				file={selected}
				readonly={selected instanceof RemoteFile}
				selectable={$commitBoxOpen && commitId === undefined}
				{commitId}
				on:close={() => {
					fileIdSelection.clear();
				}}
			/>
			<Resizer
				viewport={rsViewport}
				direction="right"
				minWidth={400}
				defaultLineColor="var(--clr-border-2)"
				on:width={(e) => {
					fileWidth = e.detail / (16 * $userSettings.zoom);
					lscache.set(fileWidthKey + branch.id, fileWidth, 7 * 1440); // 7 day ttl
					$defaultFileWidthRem = fileWidth;
				}}
			/>
		</div>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		height: 100%;
		align-items: self-start;
		flex-shrink: 0;
		user-select: none; /* here because of user-select draggable interference in board */
		position: relative;
	}

	.file-preview {
		display: flex;
		position: relative;
		height: 100%;

		overflow: hidden;
		align-items: self-start;

		padding: 12px 12px 12px 0;
	}
</style>
