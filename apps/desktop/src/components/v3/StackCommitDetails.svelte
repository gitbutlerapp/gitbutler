<script lang="ts">
	import StackDetailsCommitHeader from './StackDetailsCommitHeader.svelte';
	import StackDetailsFileList from './StackDetailsFileList.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Commit, WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		stackId: string;
		selectedCommitId: string | undefined;
		selectedCommitDetails?: Commit;
		selectedBranchDetails?: WorkspaceBranch;
	}

	let {
		selectedCommitId = $bindable(),
		stackId,
		selectedCommitDetails: commit,
		selectedBranchDetails
	}: Props = $props();

	const [projectService] = inject(ProjectService);
	const projectId = projectService.projectId;
</script>

<div class="wrapper">
	<div>
		<button type="button" class="exit-btn" onclick={() => (selectedCommitId = undefined)}>
			<Icon name="cross" />
		</button>
		{#if commit}
			<StackDetailsCommitHeader {commit} {stackId} {projectId} {selectedBranchDetails} />
		{/if}
	</div>
	<div class="body">
		{#if commit}
			<StackDetailsFileList {commit} />
		{/if}
	</div>
</div>

<style>
	.wrapper {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;

		background-color: var(--clr-bg-1);
	}

	.exit-btn {
		position: absolute;
		top: 8px;
		right: 8px;
	}

	.body {
		display: flex;
		flex-direction: column;
	}
</style>
