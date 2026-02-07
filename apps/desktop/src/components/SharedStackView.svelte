<script lang="ts">
	import BranchCard from "$components/BranchCard.svelte";
	import BranchLabel from "$components/BranchLabel.svelte";
	import CommitRow from "$components/CommitRow.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { inject } from "@gitbutler/core/context";
	import { FileListItem, HunkDiff } from "@gitbutler/ui";
	import { getColorFromBranchType } from "@gitbutler/ui/utils/getColorFromBranchType";
	import type { ReceivedStack } from "$lib/irc/receivedStacksStore.svelte";

	type Props = {
		projectId: string;
		received: ReceivedStack;
		onDismiss: () => void;
	};

	const { projectId, received, onDismiss }: Props = $props();

	const userSettings = inject(SETTINGS);

	const { payload, sender, receivedAt } = $derived(received);
	const timestamp = $derived(new Date(receivedAt).toLocaleTimeString());
</script>

<div class="shared-stack-view">
	<div class="shared-stack-header">
		<div class="shared-stack-meta text-12">
			<span class="shared-stack-sender text-semibold">{sender}</span>
			{#if payload}
				<span class="shared-stack-project">{payload.projectName}</span>
			{/if}
			<span class="shared-stack-time">{timestamp}</span>
		</div>
		<button class="shared-stack-dismiss" onclick={onDismiss} aria-label="Dismiss">✕</button>
	</div>

	<div class="shared-stack-content">
		{#each payload.branches as branch, branchIdx}
			<BranchCard
				{projectId}
				readonly
				branchName={branch.name}
				lineColor={getColorFromBranchType("LocalOnly")}
				type="pr-branch"
				trackingBranch={branch.name}
				selected={false}
			/>
			<div class="shared-branch">
				<div class="shared-branch-header">
					<BranchLabel name={branch.name} readonly />
				</div>

				{#each branch.commits as commit, commitIdx}
					<CommitRow
						type="LocalOnly"
						branchName={branch.name}
						commitId={commit.id}
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						disableCommitActions={true}
						lastCommit={commitIdx === branch.commits.length - 1}
						lastBranch={branchIdx === payload.branches.length - 1}
						first={commitIdx === 0}
						selected={true}
					>
						{#snippet changedFiles()}
							{#each commit.files as file}
								{#if file.hunks.length > 0}
									<div class="shared-file">
										<FileListItem
											filePath={file.path}
											pathFirst={$userSettings.pathFirst}
											listMode="list"
											isLast
										/>
										{#each file.hunks as hunk}
											<div class="shared-diff">
												<HunkDiff
													draggingDisabled={true}
													hideCheckboxes={true}
													filePath={file.path}
													hunkStr={hunk.diff}
													diffLigatures={$userSettings.diffLigatures}
													tabSize={$userSettings.tabSize}
													wrapText={$userSettings.wrapText}
													diffFont={$userSettings.diffFont}
													strongContrast={$userSettings.strongContrast}
													colorBlindFriendly={$userSettings.colorBlindFriendly}
													inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
												/>
											</div>
										{/each}
									</div>
								{/if}
							{/each}
						{/snippet}
					</CommitRow>
				{/each}
			</div>
		{/each}
	</div>
</div>

<style lang="postcss">
	.shared-stack-view {
		display: flex;
		flex-direction: column;
		min-width: 280px;
		height: 100%;
		border-left: 2px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.shared-stack-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 12px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.shared-stack-meta {
		display: flex;
		align-items: center;
		min-width: 0;
		overflow: hidden;
		gap: 6px;
	}

	.shared-stack-sender {
		color: var(--clr-theme-pop-element);
		white-space: nowrap;
	}

	.shared-stack-project {
		overflow: hidden;
		color: var(--clr-text-2);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.shared-stack-time {
		flex-shrink: 0;
		color: var(--clr-text-3);
		white-space: nowrap;
	}

	.shared-stack-dismiss {
		flex-shrink: 0;
		padding: 2px 6px;
		border: none;
		border-radius: var(--radius-s);
		background: none;
		color: var(--clr-text-3);
		font-size: 12px;
		cursor: pointer;

		&:hover {
			background-color: var(--clr-bg-3);
			color: var(--clr-text-1);
		}
	}

	.shared-stack-content {
		flex: 1;
		overflow-y: auto;
	}

	.shared-branch {
		border-bottom: 1px solid var(--clr-border-2);
	}

	.shared-branch-header {
		display: flex;
		align-items: center;
		padding: 10px 14px 6px;
	}

	.shared-file {
		margin-top: 2px;
	}

	.shared-diff {
		padding: 4px 0;
	}
</style>
