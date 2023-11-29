<script lang="ts">
	import type { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/draggables';
	import { dropzone } from '$lib/utils/draggable';
	import type { BaseBranch, Commit } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';

	export let projectId: string;
	export let commit: Commit;
	export let base: BaseBranch | undefined | null;
	export let isHeadCommit: boolean;
	export let isChained: boolean;
	export let readonly = false;

	export let acceptAmend: (commit: Commit) => (data: any) => boolean;
	export let acceptSquash: (commit: Commit) => (data: any) => boolean;
	export let onAmend: (data: DraggableFile | DraggableHunk) => void;
	export let onSquash: (commit: Commit) => (data: DraggableCommit) => void;
	export let resetHeadCommit: () => void;
</script>

<div class="commit-list-item flex w-full items-center gap-x-2 pb-2 pr-4">
	{#if isChained}
		<div class="line" />
	{/if}
	<div class="connector" />
	<div
		class="relative h-full flex-grow overflow-hidden"
		use:dropzone={{
			active: 'amend-dz-active',
			hover: 'amend-dz-hover',
			accepts: acceptAmend(commit),
			onDrop: onAmend
		}}
		use:dropzone={{
			active: 'squash-dz-active',
			hover: 'squash-dz-hover',
			accepts: acceptSquash(commit),
			onDrop: onSquash(commit)
		}}
	>
		<div
			class="amend-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
		>
			<div class="hover-text font-semibold">Amend</div>
		</div>
		<div
			class="squash-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
		>
			<div class="hover-text font-semibold">Squash</div>
		</div>

		<CommitCard
			{commit}
			{projectId}
			commitUrl={base?.commitUrl(commit.id)}
			{isHeadCommit}
			{resetHeadCommit}
			{readonly}
		/>
	</div>
	<!-- <div class="reset-head">
			<IconButton icon="cross-small" on:click={() => resetHeadCommit()} />
		</div> -->
</div>

<style lang="postcss">
	.commit-list-item {
		padding: 0 0 var(--space-6) var(--space-16);
		position: relative;
	}
	.line {
		position: absolute;
		top: 0;
		left: 0;
		height: 100%;
		width: 1px;
		background-color: var(--clr-theme-container-outline-light);
	}
	.connector {
		width: 16px;
		height: 18px;
		position: absolute;
		top: 0;
		left: 0;
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
		border-left: 1px solid var(--clr-theme-container-outline-light);
		border-radius: 0 0 0 8px;
	}
</style>
