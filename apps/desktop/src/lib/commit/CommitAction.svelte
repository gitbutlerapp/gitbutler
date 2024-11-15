<script lang="ts">
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { type Snippet } from 'svelte';
	import type { CellType } from '@gitbutler/ui/commitLines/types';

	interface Props {
		type: CellType;
		isLast?: boolean;
		action: Snippet;
	}

	const { type, isLast, action }: Props = $props();
</script>

<div class="action-row" style:--commit-color={getColorFromBranchType(type)}>
	<div class="commit-line-wrapper">
		<div class="commit-line" class:dashed={isLast}></div>
	</div>

	<div class="action">
		{@render action()}
	</div>
</div>

<style lang="postcss">
	.action-row {
		position: relative;
		display: flex;
		background-color: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-3);
		overflow: hidden;
	}

	.background-color {
		background-color: var(--clr-bg-2);
	}

	.action {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding-top: 14px;
		padding-right: 14px;
		padding-bottom: 14px;
	}

	.commit-line-wrapper {
		position: relative;
		margin-left: 20px;
		margin-right: 20px;
	}

	/* MODIFIERS */
	.bottom-border {
		border-bottom: 1px solid var(--clr-border-2);
	}

	.sticky {
		position: sticky;
		bottom: 0;
	}

	.sticky-z-index {
		z-index: var(--z-lifted);
	}

	.not-in-viewport {
		box-shadow: 0 0 0 1px var(--clr-border-2);
	}
</style>
