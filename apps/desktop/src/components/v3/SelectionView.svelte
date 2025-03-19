<script lang="ts">
	import SelectedChange from './SelectedChange.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [idSelection] = inject(IdSelection);
	const selection = $derived(idSelection.values());
</script>

<div class="selection-view">
	<ScrollableContainer wide>
		{#each selection as selectedFile, index ('selection-view-' + selectedFile.path + index)}
			<SelectedChange {projectId} {selectedFile} {index} />
		{/each}
	</ScrollableContainer>
</div>

<style>
	.selection-view {
		flex-grow: 1;
		height: 100%;
		overflow: hidden;
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
	}
</style>
