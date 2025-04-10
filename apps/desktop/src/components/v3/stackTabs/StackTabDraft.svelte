<script lang="ts">
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';

	const [uiState] = inject(UiState);
	const branchNameDraft = $derived(uiState.global.draftBranchName?.current);
</script>

<div class="new-stack-btn">
	<div class="tab-icon">
		<Icon name="branch-small" verticalAlign="top" />
	</div>
	<div class="text-12 text-semibold tab-name">{branchNameDraft}</div>
</div>

<style lang="postcss">
	.new-stack-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px dashed var(--clr-border-2);
		border-bottom: none;
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		height: 100%;
		padding: 12px 15px;
		background: var(--clr-stack-tab-inactive);
		color: var(--clr-text-2);
		--plus-icon-opacity: 0.8;
		transition:
			color var(--transition-fast),
			background var(--transition-fast);
		gap: 10px;
	}

	.tab-icon {
		color: var(--clr-text-2);
		display: flex;
		align-items: center;
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		border-radius: var(--radius-s);
		width: 16px;
		height: 16px;
		line-height: 16px;
	}

	.tab-name {
		position: relative;
		width: 100%;
		white-space: nowrap;
		margin-right: calc(var(--truncation-gradient-width));

		/* overflow: hidden; */
	}
</style>
