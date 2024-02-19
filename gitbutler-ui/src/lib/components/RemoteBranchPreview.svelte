<script lang="ts">
	import BranchPreviewHeader from './BranchPreviewHeader.svelte';
	import FileCard from './FileCard.svelte';
	import Resizer from './Resizer.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import CommitCard from '$lib/components/CommitCard.svelte';
	import { type SettingsStore, SETTINGS_CONTEXT } from '$lib/settings/userSettings';
	import { getRemoteBranchData } from '$lib/stores/remoteBranches';
	import { Ownership } from '$lib/vbranches/ownership';
	import lscache from 'lscache';
	import { marked } from 'marked';
	import { getContext, onMount } from 'svelte';
	import { writable } from 'svelte/store';
	import type { PullRequest } from '$lib/github/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { AnyFile, BaseBranch, RemoteBranch } from '$lib/vbranches/types';

	export let base: BaseBranch | undefined | null;
	export let branch: RemoteBranch;
	export let projectId: string;
	export let projectPath: string;
	export let branchController: BranchController;
	export let pr: PullRequest | undefined;

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'branchPreviewLaneWidth';
	const selectedFiles = writable<AnyFile[]>([]);
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let rsViewport: HTMLDivElement;
	let laneWidth: number;

	$: selectedOwnership = writable(Ownership.default());
	$: selected = setSelected($selectedFiles);

	function setSelected(files: AnyFile[]) {
		if (files.length == 0) return undefined;
		return files[0];
	}

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey);
	});

	var renderer = new marked.Renderer();
	renderer.link = function (href, title, text) {
		if (!title) title = text;
		return '<a target="_blank" href="' + href + '" title="' + title + '">' + text + '</a>';
	};
</script>

<div class="base">
	<div
		class="base__left"
		bind:this={rsViewport}
		style:width={`${laneWidth || defaultBranchWidthRem}rem`}
	>
		<ScrollableContainer wide>
			<div class="branch-preview">
				<BranchPreviewHeader {projectId} {base} {branch} {pr} {branchController} />
				{#if pr?.body}
					<div class="card">
						<div class="card__header">PR Description</div>
						<div class="card__content">
							{@html marked.parse(pr.body, { renderer })}
						</div>
					</div>
				{/if}
				{#await getRemoteBranchData({ projectId, refname: branch.name }) then branchData}
					{#if branchData.commits && branchData.commits.length > 0}
						<div class="flex w-full flex-col gap-y-2">
							{#each branchData.commits as commit (commit.id)}
								<CommitCard
									{commit}
									{projectId}
									{selectedFiles}
									{branchController}
									commitUrl={base?.commitUrl(commit.id)}
								/>
							{/each}
						</div>
					{/if}
				{/await}
			</div>
		</ScrollableContainer>
		<Resizer
			viewport={rsViewport}
			direction="right"
			minWidth={320}
			on:width={(e) => {
				laneWidth = e.detail / (16 * $userSettings.zoom);
				lscache.set(laneWidthKey, laneWidth, 7 * 1440); // 7 day ttl
			}}
		/>
	</div>
	<div class="base__right">
		{#if selected}
			<FileCard
				conflicted={selected.conflicted}
				branchId={'blah'}
				file={selected}
				{projectPath}
				{branchController}
				{selectedOwnership}
				isUnapplied={false}
				readonly={true}
				on:close={() => {
					const selectedId = selected?.id;
					selectedFiles.update((fileIds) => fileIds.filter((file) => file.id != selectedId));
				}}
			/>
		{/if}
	</div>
</div>

<style lang="postcss">
	.base {
		display: flex;
		flex-grow: 1;
		overflow-x: auto;
	}
	.base__left {
		display: flex;
		flex-grow: 0;
		flex-shrink: 0;
		overflow-x: hidden;
		position: relative;
	}
	.base__right {
		display: flex;
		overflow-x: auto;
		align-items: flex-start;
		padding: var(--space-12) var(--space-12) var(--space-12) var(--space-6);
	}

	.branch-preview {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
		margin: var(--space-12) var(--space-6) var(--space-12) var(--space-12);
	}
</style>
