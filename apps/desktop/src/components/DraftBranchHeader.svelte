<script lang="ts">
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import BranchLabel from '$components/BranchLabel.svelte';
	import CommitGoesHere from '$components/CommitGoesHere.svelte';
	import { TestId } from '@gitbutler/ui';

	type Props = {
		branchName: string;
		lineColor: string;
		mode?: 'commit' | 'codegen';
		isCommitting?: boolean;
		isCommitTarget?: boolean;
		commitId?: string;
		updateBranchName: (name: string) => void;
		isUpdatingName: boolean;
		failedToUpdateName: boolean;
	};

	const {
		branchName,
		lineColor,
		mode = 'commit',
		isCommitting = false,
		commitId,
		updateBranchName,
		isUpdatingName,
		failedToUpdateName
	}: Props = $props();
</script>

<div class="header-wrapper">
	<div
		data-testid={TestId.BranchHeader}
		data-testid-branch-header={branchName}
		class="branch-header"
		class:commiting={isCommitting}
		data-remove-from-panning
	>
		<div class="branch-header__content">
			<div class="branch-header__title text-14 text-bold">
				<div class="branch-header__title-content flex gap-6">
					<BranchHeaderIcon color={lineColor} iconName="branch-local" />
					<BranchLabel
						name={branchName}
						fontSize="15"
						disabled={isUpdatingName}
						error={failedToUpdateName}
						readonly={false}
						onChange={(name) => updateBranchName(name)}
					/>
				</div>
			</div>

			<p class="text-12 text-body branch-header__empty-state">
				A new branch will be created for your {mode === 'commit' ? 'commit' : 'AI session'}.
				<br />
				Click the name to rename it now or later.
			</p>
		</div>
	</div>

	{#if isCommitting}
		<CommitGoesHere {commitId} selected={true} draft last={false} />
	{/if}
</div>

<style lang="postcss">
	.header-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.branch-header {
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: center;
		justify-content: flex-start;
		padding-right: 12px;
		padding-left: 12px;
		overflow: hidden;
		border-bottom: none;
		background-color: var(--clr-bg-2);
	}

	.branch-header__title {
		display: flex;
		flex-grow: 1;
		align-items: center;
		justify-content: space-between;
		min-width: 0;
		gap: 4px;
	}

	.branch-header__title-content {
		flex-grow: 1;
		align-items: center;
		min-width: 0;
	}

	.branch-header__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		padding: 14px 0;
		overflow: hidden;
		gap: 8px;
		text-overflow: ellipsis;
	}

	.branch-header__empty-state {
		color: var(--clr-text-2);
		opacity: 0.8;
	}
</style>
