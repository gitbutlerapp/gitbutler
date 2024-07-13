<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import { Project } from '$lib/backend/projects';
	import { projectLaneCollapsed } from '$lib/config/config';
	import FileCard from '$lib/file/FileCard.svelte';
	import { createHostedGitChecksMonitorStore } from '$lib/gitHost/interface/hostedGitChecksMonitor';
	import { getHostedGitListingServiceStore } from '$lib/gitHost/interface/hostedGitListingService';
	import { createHostedGitPrMonitorStore } from '$lib/gitHost/interface/hostedGitPrMonitor';
	import { createHostedGitPrServiceStore } from '$lib/gitHost/interface/hostedGitPrService';
	import { getHostedGitServiceStore } from '$lib/gitHost/interface/hostedGitService';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import {
		getContext,
		getContextStoreBySymbol,
		createContextStore,
		getContextStore
	} from '$lib/utils/context';
	import {
		createIntegratedCommitsContextStore,
		createLocalCommitsContextStore,
		createLocalAndRemoteCommitsContextStore,
		createRemoteCommitsContextStore
	} from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { RemoteFile, Branch, BaseBranch } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	const { branch }: { branch: Branch } = $props();

	const baseBranch = getContextStore(BaseBranch);

	const hostedGitService = getHostedGitServiceStore();
	const baseBranchName = $derived($baseBranch.shortName);
	const upstreamName = $derived(branch.upstreamName);

	// BRANCH SERVICE
	const prService = createHostedGitPrServiceStore(undefined);
	$effect(() =>
		prService.set(
			upstreamName ? $hostedGitService?.prService(baseBranchName, upstreamName) : undefined
		)
	);

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getHostedGitListingServiceStore();
	const prs = $derived($hostedListingServiceStore?.prs);

	const listedPr = $derived($prs?.find((pr) => pr.sourceBranch === branch.upstreamName));
	const sourceBranch = $derived(listedPr?.sourceBranch);
	const prNumber = $derived(listedPr?.number);

	const hostedGitPrMonitorStore = createHostedGitPrMonitorStore(undefined);
	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	$effect(() => hostedGitPrMonitorStore.set(prMonitor));

	const hostedGitChecksMonitorStore = createHostedGitChecksMonitorStore(undefined);
	const checksMonitor = $derived(
		sourceBranch ? $hostedGitService?.checksMonitor(sourceBranch) : undefined
	);
	$effect(() => hostedGitChecksMonitorStore.set(checksMonitor));

	// BRANCH
	const branchStore = createContextStore(Branch, branch);
	const ownershipStore = createContextStore(Ownership, Ownership.fromBranch(branch));
	const branchFiles = writable(branch.files);

	$effect(() => {
		branchStore.set(branch);
		ownershipStore.set(Ownership.fromBranch(branch));
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
	const selectedFile = $derived(fileIdSelection.selectedFile);
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
	<BranchCard {commitBoxOpen} {isLaneCollapsed} />

	{#await $selectedFile then selected}
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
					selectable={$commitBoxOpen}
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
	{/await}
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
