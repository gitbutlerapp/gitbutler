<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { projectLaneCollapsed } from '$lib/config/config';
	import FileCard from '$lib/file/FileCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { createGitHostChecksMonitorStore } from '$lib/gitHost/interface/gitHostChecksMonitor';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { createGitHostPrMonitorStore } from '$lib/gitHost/interface/gitHostPrMonitor';
	import { createGitHostPrServiceStore } from '$lib/gitHost/interface/gitHostPrService';
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
	import { RemoteFile, VirtualBranch } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	const { branch }: { branch: VirtualBranch } = $props();

	const baseBranch = getContextStore(BaseBranch);

	const gitHost = getGitHost();
	const baseBranchName = $derived($baseBranch.shortName);
	const upstreamName = $derived(branch.upstreamName);

	// BRANCH SERVICE
	const prService = createGitHostPrServiceStore(undefined);
	$effect(() =>
		prService.set(
			upstreamName && baseBranchName ? $gitHost?.prService(baseBranchName, upstreamName) : undefined
		)
	);

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

<div class="wrapper" data-testid="branch-{branch.name}" data-tauri-drag-region>
	<BranchCard {commitBoxOpen} {isLaneCollapsed} />

	{#await $selectedFile then [commitId, selected]}
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
