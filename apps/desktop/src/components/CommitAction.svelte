<script lang="ts">
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { type Snippet } from 'svelte';
	import type { CellType } from '@gitbutler/ui/components/commitLines/types';

	interface Props {
		type: CellType;
		isLast?: boolean;
		action: Snippet;
	}

	const { type, isLast, action }: Props = $props();
</script>

<div class="action-row" class:is-last={isLast} style:--commit-color={getColorFromBranchType(type)}>
	<div class="commit-line-wrapper">
		<div class="commit-line" class:dashed={isLast}></div>
	</div>

	<div class="action">
		{@render action()}
	</div>
</div>

<style lang="postcss">
	.action-row {
		display: flex;
		position: relative;
		overflow: hidden;
		border-top: 1px solid var(--clr-border-3);
		background-color: var(--clr-bg-1);
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
		margin-right: 20px;
		margin-left: 20px;
	}
</style>
