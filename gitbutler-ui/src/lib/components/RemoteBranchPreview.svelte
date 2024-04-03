<script lang="ts">
	import BranchPreviewHeader from './BranchPreviewHeader.svelte';
	import FileCard from './FileCard.svelte';
	import Resizer from './Resizer.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import { Project } from '$lib/backend/projects';
	import CommitCard from '$lib/components/CommitCard.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getRemoteBranchData } from '$lib/stores/remoteBranches';
	import { getContext, getContextStore, getContextStoreBySymbol } from '$lib/utils/context';
	import { createSelectedFileIds, createSelectedFiles } from '$lib/vbranches/contexts';
	import { FileSelection } from '$lib/vbranches/fileSelection';
	import { BaseBranch, type RemoteBranch } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { marked } from 'marked';
	import { onMount } from 'svelte';
	import type { PullRequest } from '$lib/github/types';

	export let branch: RemoteBranch;
	export let pr: PullRequest | undefined;

	const project = getContext(Project);
	const baseBranch = getContextStore(BaseBranch);

	createSelectedFileIds(new FileSelection());
	const selectedFiles = createSelectedFiles([]);

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'branchPreviewLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let rsViewport: HTMLDivElement;
	let laneWidth: number;

	$: selected = $selectedFiles.length == 1 ? $selectedFiles[0] : undefined;

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
				<BranchPreviewHeader base={$baseBranch} {branch} {pr} />
				{#if pr?.body}
					<div class="card">
						<div class="card__header text-base-body-14 text-semibold">PR Description</div>
						<div class="markdown card__content text-base-body-13">
							{@html marked.parse(pr.body, { renderer })}
						</div>
					</div>
				{/if}
				{#await getRemoteBranchData(project.id, branch.name) then branchData}
					{#if branchData.commits && branchData.commits.length > 0}
						<div class="branch-preview__commits-list">
							{#each branchData.commits as commit (commit.id)}
								<CommitCard {commit} commitUrl={$baseBranch?.commitUrl(commit.id)} />
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
				file={selected}
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
		padding: var(--size-12) var(--size-12) var(--size-12) var(--size-6);
	}

	.branch-preview {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		margin: var(--size-12) var(--size-6) var(--size-12) var(--size-12);
	}

	.card__content {
		color: var(--clr-scale-ntrl-30);
	}

	.branch-preview__commits-list {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}
</style>
