<script lang="ts">
	import Resizer from '$lib/resizer/Resizer.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import lscache from 'lscache';
	import { quintOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	interface Props {
		branchId?: string;
		branchCard: Snippet;
		isLaneCollapsed: boolean;
		fileCard?: Snippet;
		fileCardExpanded: boolean;
	}

	const {
		branchId = 'unset',
		branchCard,
		isLaneCollapsed,
		fileCard,
		fileCardExpanded
	}: Props = $props();

	const SEVEN_DAYS = 1000 * 60 * 60 * 24 * 7;

	const BRANCH_CARD_WIDTH_KEY = '@gitbutler/ui-fileWidthPx';

	function getBranchCardInitialWidthPx() {
		return parseInt(lscache.get(BRANCH_CARD_WIDTH_KEY + '-' + branchId) || '480');
	}

	function setBranchCardInitialWidthPx(px: number) {
		lscache.set(BRANCH_CARD_WIDTH_KEY + '-' + branchId, px.toString(), SEVEN_DAYS);
	}

	// TODO: Make this on a per branch basis
	let branchCardWidthPx = $state(getBranchCardInitialWidthPx());
	let branchCardViewport = $state<HTMLElement>();

	const FILE_WIDTH_KEY = '@gitbutler/ui-fileWidthPx';

	function getFilesInitialWidthPx() {
		return parseInt(lscache.get(FILE_WIDTH_KEY + '-' + branchId) || '480');
	}

	function setFilesInitialWidthPx(px: number) {
		lscache.set(FILE_WIDTH_KEY + '-' + branchId, px.toString(), SEVEN_DAYS);
	}

	let fileWidthPx = $state(getFilesInitialWidthPx());
	let filesViewport = $state<HTMLElement>();

	// File card preview should be shown when a file card snippet is provided,
	// the fileCardExpanded boolean is true, and when the branch card is not
	// collapsed
	const showFilePreview = $derived(fileCard && fileCardExpanded && !isLaneCollapsed);
</script>

<div class="wrapper">
	<div
		class="branch-preview"
		bind:this={branchCardViewport}
		style:width={pxToRem(branchCardWidthPx)}
	>
		{@render branchCard()}

		{#if !isLaneCollapsed}
			<Resizer
				viewport={branchCardViewport}
				direction="right"
				minWidth={380}
				defaultLineColor={fileCardExpanded ? 'transparent' : 'var(--clr-border-2)'}
				on:width={(e) => {
					branchCardWidthPx = e.detail;
					setBranchCardInitialWidthPx(fileWidthPx);
				}}
			/>
		{/if}
	</div>
	{#if showFilePreview}
		<div
			class="file-preview"
			bind:this={filesViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={pxToRem(fileWidthPx)}
		>
			{@render fileCard!()}
			<Resizer
				viewport={filesViewport}
				direction="right"
				minWidth={400}
				defaultLineColor="var(--clr-border-2)"
				on:width={(e) => {
					fileWidthPx = e.detail;
					setFilesInitialWidthPx(fileWidthPx);
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

	.branch-preview {
		position: relative;
		display: flex;
		height: 100%;
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
