<script lang="ts">
	import UnifiedDiffView from './UnifiedDiffView.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [idSelection, worktreeService] = inject(IdSelection, WorktreeService);
	const selection = $derived(idSelection.values());
</script>

<div class="selection-view">
	<ScrollableContainer wide>
		{#each selection as selectedFile (selectedFile.path)}
			{@const changeResult = worktreeService.getChange(projectId, selectedFile.path).current}
			<ReduxResult result={changeResult}>
				{#snippet children(change)}
					<FileListItemWrapper {projectId} {change} sticky />
					<UnifiedDiffView {projectId} {change} selectable />
				{/snippet}
			</ReduxResult>
		{/each}
	</ScrollableContainer>
</div>

<style>
	.selection-view {
		flex-grow: 1;
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-2);
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
	}
</style>
