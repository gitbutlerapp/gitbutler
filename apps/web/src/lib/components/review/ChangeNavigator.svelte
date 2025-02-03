<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

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
	<Button
		disabled={!previousPatchId}
		kind="outline"
		class="navigate-prev"
		onclick={handleNavigateBack}
		icon="chevron-left"
	></Button>
	<div class="indicator text-12 text-semibold">
		Patch {patchIds?.length - index}/{patchIds?.length}
	</div>
	<Button
		disabled={!nextPatchId}
		kind="outline"
		class="navigate-next"
		onclick={handleNavigateForward}
		icon="chevron-right"
	></Button>
</div>

<style lang="postcss">
	.change-navigator {
		display: flex;
	}

	:global(.change-navigator .navigate-prev) {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
		border-right: none;
	}

	:global(.change-navigator .navigate-next) {
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;
		border-left: none;
	}

	.indicator {
		--label-clr: var(--clr-text-1);
		--btn-border-clr: var(--clr-btn-ntrl-outline);
		--btn-border-opacity: var(--opacity-btn-outline);

		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0 10px;
		color: var(--label-clr);

		border: 1px solid
			color-mix(
				in srgb,
				var(--btn-border-clr, transparent),
				transparent calc((1 - var(--btn-border-opacity, 1)) * 100%)
			);
	}
</style>
