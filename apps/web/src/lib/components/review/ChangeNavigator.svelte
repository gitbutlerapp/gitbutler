<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		goToPatch: (patchId: string) => void;
		currentPatchId: string;
		patchIds: string[];
	}

	const { patchIds, currentPatchId, goToPatch }: Props = $props();
	const index = $derived(patchIds.indexOf(currentPatchId));
	const previousPatchId = $derived(patchIds[index + 1]);
	const nextPatchId = $derived(patchIds[index - 1]);

	function handleNavigateBack() {
		if (previousPatchId) {
			goToPatch(previousPatchId);
		}
	}

	function handleNavigateForward() {
		if (nextPatchId) {
			goToPatch(nextPatchId);
		}
	}
</script>

<div class="change-navigator">
	<button
		type="button"
		disabled={!previousPatchId}
		class="navigate-prev"
		onclick={handleNavigateBack}><Icon name="chevron-left" /></button
	>
	<div class="indicator">Patch {patchIds?.length - index}/{patchIds?.length}</div>
	<button
		type="button"
		disabled={!nextPatchId}
		class="navigate-next"
		onclick={handleNavigateForward}><Icon name="chevron-right" /></button
	>
</div>

<style lang="postcss">
	.change-navigator {
		display: flex;
	}

	.navigate-prev {
		cursor: pointer;
		display: flex;
		height: var(--button, 28px);
		padding: 4px 6px;
		justify-content: center;
		align-items: center;
		gap: 4px;

		border-radius: var(--m, 6px) 0px 0px var(--m, 6px);
		border: 1px solid var(--border-2, #d4d0ce);

		&:disabled {
			pointer-events: none;
		}
	}

	.navigate-next {
		cursor: pointer;
		display: flex;
		height: var(--button, 28px);
		padding: 4px 6px;
		justify-content: center;
		align-items: center;
		gap: 4px;

		border-radius: 0px var(--m, 6px) var(--m, 6px) 0px;
		border: 1px solid var(--border-2, #d4d0ce);

		&:disabled {
			pointer-events: none;
		}
	}

	.indicator {
		display: flex;
		height: var(--button, 28px);
		padding: 4px 6px;
		justify-content: center;
		align-items: center;
		gap: 4px;

		color: var(--text-1, #1a1614);

		/* base/12-semibold */
		font-family: var(--font-family-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: 500;
		line-height: 120%; /* 14.4px */

		border-top: 1px solid var(--border-2, #d4d0ce);
		border-bottom: 1px solid var(--border-2, #d4d0ce);
	}
</style>
