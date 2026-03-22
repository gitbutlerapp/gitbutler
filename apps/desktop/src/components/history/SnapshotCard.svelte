<script lang="ts">
	import FileListItems from "$components/files/FileListItems.svelte";
	import FileListProvider from "$components/files/FileListProvider.svelte";
	import SnapshotAttachment from "$components/history/SnapshotAttachment.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { createdOnDay, HISTORY_SERVICE } from "$lib/history/history";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { toHumanReadableTime } from "$lib/utils/time";
	import { inject } from "@gitbutler/core/context";
	import { Button, Icon, ScrollableContainer, type IconName } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import type { Snapshot, SnapshotDetails } from "$lib/history/types";

	interface Props {
		entry: Snapshot;
		isWithinRestore?: boolean;
		restoring?: boolean;
		onRestoreClick: () => void;
		onDiffClick: (filePath: string) => void;
		projectId: string;
	}

	const {
		projectId,
		entry,
		isWithinRestore = true,
		restoring = false,
		onRestoreClick,
		onDiffClick,
	}: Props = $props();

	function getShortSha(sha: string | undefined) {
		if (!sha) return "";

		return `${sha.slice(0, 7)}`;
	}

	function createdOnDayAndTime(epoch: number) {
		const date = new Date(epoch);
		return `${createdOnDay(date)}, ${toHumanReadableTime(date)}`;
	}

	function camelToTitleCase(str: string | undefined) {
		if (!str) return "";
		const lowerCaseStr = str.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
		return lowerCaseStr.charAt(0).toUpperCase() + lowerCaseStr.slice(1);
	}

	function mapOperation(snapshotDetails: SnapshotDetails | undefined): {
		text: string;
		icon?: IconName;
		commitMessage?: string;
	} {
		if (!snapshotDetails) return { text: "", icon: "commit" };

		function trailer(key: string) {
			return snapshotDetails?.trailers.find((t) => t.key === key)?.value;
		}
		function entryTrailer(key: string) {
			return entry.details?.trailers.find((t) => t.key === key)?.value;
		}

		switch (snapshotDetails.operation) {
			// REMOVE
			case "DeleteBranch":
				return { text: `Delete branch "${entryTrailer("name")}"`, icon: "cross" };
			case "DiscardLines":
			case "DiscardHunk":
			case "DiscardFile":
				return { text: camelToTitleCase(snapshotDetails.operation), icon: "cross" };

			// ADD
			case "CreateBranch":
				return { text: `Create branch "${trailer("name")}"`, icon: "plus" };
			case "CreateCommit":
			case "InsertBlankCommit":
				return {
					text:
						snapshotDetails.operation === "CreateCommit"
							? `Create commit ${getShortSha(entryTrailer("sha"))}`
							: "Insert blank commit",
					icon: "plus",
					commitMessage: entryTrailer("message"),
				};

			// EDIT
			case "UpdateBranchName":
				return {
					text: `Renamed branch "${trailer("before")}" to "${trailer("after")}"`,
					icon: "edit",
				};
			case "UpdateBranchRemoteName":
				return {
					text: `Update branch remote name "${trailer("before")}" to "${trailer("after")}"`,
					icon: "edit",
				};
			case "AmendCommit":
				return { text: "Amend commit", icon: "edit" };
			case "UpdateCommitMessage":
				return { text: "Update commit message", icon: "edit" };
			case "EnterEditMode":
				return { text: "Enter Edit Mode", icon: "edit" };

			// BRANCH
			case "ApplyBranch":
				return { text: `Apply branch "${entryTrailer("name")}"`, icon: "branch" };
			case "UnapplyBranch":
				return { text: `Unapply branch "${trailer("branch")}"`, icon: "branch" };
			case "ReorderBranches":
				return {
					text: `Reorder branches "${trailer("before")}" and "${trailer("after")}"`,
					icon: "branch",
				};
			case "SelectDefaultVirtualBranch":
				return { text: `Select default virtual branch "${trailer("after")}"`, icon: "branch" };
			case "SetBaseBranch":
				return { text: "Set base branch", icon: "branch" };
			case "GenericBranchUpdate":
				return { text: "Generic branch update", icon: "branch" };
			case "SplitBranch":
				return { text: "Split branch", icon: "branch" };
			case "MoveBranch":
			case "TearOffBranch":
				return { text: camelToTitleCase(snapshotDetails.operation), icon: "branch" };

			// COMMIT
			case "UndoCommit":
				return {
					text: `Undo commit ${getShortSha(entryTrailer("sha"))}`,
					icon: "undo",
					commitMessage: entryTrailer("message"),
				};
			case "SquashCommit":
				return { text: "Squash commit", icon: "commit-arrow-down" };
			case "MoveCommit":
			case "ReorderCommit":
				return { text: camelToTitleCase(snapshotDetails.operation), icon: "commit" };
			case "MoveCommitFile":
				return { text: "Move commit file", icon: "commit" };
			case "Absorb":
				return { text: "Absorb changes into commit", icon: "commit-absorb" };
			case "AutoCommit":
				return { text: "Auto commit changes", icon: "commit-ai" };

			// FILE
			case "MoveHunk":
				return { text: `Move hunk to "${entryTrailer("name")}"`, icon: "file" };
			case "FileChanges":
				return { text: "File changes", icon: "file" };

			// OTHER
			case "MergeUpstream":
				return { text: "Merge upstream", icon: "pr-tick" };
			case "UpdateWorkspaceBase":
				return { text: "Update workspace base", icon: "refresh" };
			case "RestoreFromSnapshot":
				return { text: "Revert snapshot" };
			case "OnDemandSnapshot":
				return {
					text: snapshotDetails.body
						? `Manual snapshot: ${snapshotDetails.body}`
						: "Manual snapshot",
					icon: "camera",
				};
			default:
				return { text: snapshotDetails.operation, icon: "commit" };
		}
	}

	const isRestoreSnapshot = entry.details?.operation === "RestoreFromSnapshot";
	const error = entry.details?.trailers.find((t) => t.key === "error")?.value;

	const operation = mapOperation(entry.details);

	const modeService = inject(MODE_SERVICE);
	const mode = $derived(modeService.mode(projectId));

	const historyService = inject(HISTORY_SERVICE);
	const snapshotDiff = $derived(historyService.snapshotDiff({ projectId, snapshotId: entry.id }));
</script>

<div
	class="snapshot-card show-restore-on-hover"
	class:restored-snapshot={isRestoreSnapshot || isWithinRestore}
	use:focusable={{ focusable: true }}
>
	<div class="snapshot-right-container">
		<div class="restore-btn">
			<Button
				size="tag"
				kind="outline"
				tooltip="Restores GitButler and your files to the state before this operation. Revert actions can also be undone."
				onclick={() => {
					onRestoreClick();
				}}
				disabled={restoring || mode.response?.type !== "OpenWorkspace"}
				loading={restoring}>Revert</Button
			>
		</div>
		<span class="snapshot-time text-11">
			{toHumanReadableTime(entry.createdAt)}
		</span>
	</div>

	<div class="snapshot-line">
		{#if isRestoreSnapshot}
			<img src="/images/history/restore-icon.svg" alt="" />
		{:else if operation.icon}
			<Icon name={operation.icon} />
		{/if}
	</div>

	<div class="snapshot-content">
		<div class="snapshot-details">
			<h4 class="snapshot-title text-13 text-body text-semibold">
				<span>{operation.text}</span>
				<span class="snapshot-sha text-12 text-body"> • {getShortSha(entry.id)}</span>
			</h4>

			{#if operation.commitMessage}
				<p class="text-12 text-body snapshot-commit-message">
					<span>Message:</span>
					{operation.commitMessage}
				</p>
			{/if}
		</div>

		<ReduxResult result={snapshotDiff.result} {projectId}>
			{#snippet children(files)}
				{#if files.length > 0 && !isRestoreSnapshot}
					<FileListProvider
						changes={files}
						selectionId={{ type: "snapshot", snapshotId: entry.id }}
						allowUnselect={false}
					>
						<SnapshotAttachment
							foldable={files.length > 2}
							foldedAmount={files.length}
							foldedHeight="7rem"
						>
							<ScrollableContainer>
								<FileListItems
									{projectId}
									mode="list"
									onselect={(change) => onDiffClick(change.path)}
								/>
							</ScrollableContainer>
						</SnapshotAttachment>
					</FileListProvider>
				{/if}
			{/snippet}
		</ReduxResult>

		{#if isRestoreSnapshot}
			<SnapshotAttachment>
				<div class="restored-attacment">
					<Icon name="commit" />
					<div class="restored-attacment__content">
						<h4 class="text-13 text-semibold">
							{camelToTitleCase(
								entry.details?.trailers.find((t) => t.key === "restored_operation")?.value,
							)}
						</h4>
						<span class="restored-attacment__details text-12">
							{getShortSha(entry.details?.trailers.find((t) => t.key === "restored_from")?.value)} •
							{createdOnDayAndTime(
								parseInt(
									entry.details?.trailers.find((t) => t.key === "restored_date")?.value || "",
								),
							)}
						</span>
					</div>
				</div>
			</SnapshotAttachment>
		{/if}
		{#if error}
			<div class="error-text text-12 text-body">
				{error}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	/* SNAPSHOT CARD */
	.snapshot-card {
		display: flex;
		position: relative;
		padding: 10px 14px 8px 14px;
		overflow: hidden;
		gap: 12px;
		background-color: var(--clr-bg-1);
		transition: padding 0.2s;
	}

	.show-restore-on-hover {
		&:hover {
			background-color: var(--hover-bg-1);
			& .restore-btn {
				display: flex;
			}
			& .snapshot-time {
				display: none;
			}
		}
	}

	.show-restore-on-hover:global(.focused) {
		background-color: var(--hover-bg-1);
		& .restore-btn {
			display: flex;
		}
		& .snapshot-time {
			display: none;
		}
	}

	.snapshot-right-container {
		display: flex;
		justify-content: flex-end;
		width: 60px;
	}

	.restore-btn {
		display: none;
	}

	.snapshot-time {
		margin-top: 2px;
		color: var(--clr-text-2);
		line-height: 1.8;
		text-align: right;
	}

	.snapshot-line {
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: center;
		margin-top: 3px;

		&::after {
			position: absolute;
			top: 24px;
			width: 1px;
			height: calc(100% - 14px);
			min-height: 8px;
			background-color: var(--clr-border-2);
			content: "";
		}
	}

	/* CARD CONTENT */
	.snapshot-content {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: flex-start;
		min-height: var(--size-tag);
		overflow: hidden;
		gap: 6px;
	}

	.snapshot-details {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		width: 100%;
		margin-top: 2px;
		margin-bottom: 4px;
		gap: 6px;
	}

	.snapshot-title {
		flex: 1;
	}

	.snapshot-commit-message {
		margin-bottom: 2px;
		color: var(--clr-text-2);

		& span {
			color: var(--clr-text-3);
		}
	}

	.snapshot-sha {
		color: var(--clr-text-3);
		white-space: nowrap;
	}

	/* ATTACHMENT RESTORE */
	.restored-attacment {
		display: flex;
		padding: 12px;
		gap: 8px;
	}

	.restored-attacment__content {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.restored-attacment__details {
		color: var(--clr-text-2);
	}

	/* RESTORED  */
	.restored-snapshot {
		background-color: var(--clr-bg-2);
	}

	/* --- */
	.error-text {
		display: flex;
		width: 100%;
		padding: 6px 10px;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-danger-bg);
		color: var(--clr-theme-danger-element);
	}
</style>
