<script lang="ts">
	import UnifiedDiffView from './UnifiedDiffView.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';

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
					<FileListItem filePath={selectedFile.path} />
					<UnifiedDiffView {projectId} {change} selectable />
				{/snippet}
			</ReduxResult>
		{/each}
	</ScrollableContainer>
</div>

<style lang="postcss">
	.selection-view {
		width: 100%;
		overflow: hidden;
	}
</style>
