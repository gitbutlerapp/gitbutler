<script lang="ts">
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { type Snippet } from 'svelte';
	import type { CellType } from '@gitbutler/ui/components/commitLines/types';

	interface Props {
		type: CellType;
		isLast?: boolean;
		action: Snippet;
		kind?: 'default' | 'warning';
	}

	const { type, isLast, action, kind = 'default' }: Props = $props();
</script>

<div
	class="action-row {kind}"
	class:is-last={isLast}
	style:--commit-color={getColorFromBranchType(type)}
>
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
		border-bottom: 1px solid var(--clr-border-2);

		&.default {
			background-color: var(--clr-bg-1);
		}
		&.warning {
			background-color: var(--clr-theme-warn-bg);
		}
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
