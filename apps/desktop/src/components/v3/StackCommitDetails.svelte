<script lang="ts">
	import StackDetailsCommitHeader from './StackDetailsCommitHeader.svelte';
	import StackDetailsFileList from './StackDetailsFileList.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { Commit, WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		stackId: string;
		selectedCommitDetails?: Commit;
		selectedBranchDetails?: WorkspaceBranch;
		onClose: () => void;
	}

	let { stackId, selectedCommitDetails: commit, selectedBranchDetails, onClose }: Props = $props();

	const [projectService] = inject(ProjectService);
	const projectId = projectService.projectId;
</script>

<div class="stack-commit-details">
	<Button type="button" kind="ghost" class="exit-btn" icon="cross" size="tag" onclick={onClose}
	></Button>
	{#if commit}
		<StackDetailsCommitHeader {commit} {stackId} {projectId} {selectedBranchDetails} />
	{/if}

	<div class="body">
		{#if commit}
			<StackDetailsFileList {commit} />
		{/if}
	</div>
</div>

<style>
	.stack-commit-details {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;

		background-color: var(--clr-bg-1);

		:global(.exit-btn) {
			position: absolute;
			top: 8px;
			right: 8px;
		}
	}

	.body {
		display: flex;
		flex-direction: column;
	}
</style>
