<script lang="ts">
	import CommitContextMenu from "$components/commit/CommitContextMenu.svelte";
	import CommitDetails from "$components/commit/CommitDetails.svelte";
	import CommitMessageEditor from "$components/commit/CommitMessageEditor.svelte";
	import CommitTitle from "$components/commit/CommitTitle.svelte";
	import { isLocalAndRemoteCommit } from "$components/lib";
	import Drawer from "$components/shared/Drawer.svelte";
	import { splitMessage } from "$lib/commits/commitMessage";
	import { rewrapCommitMessage } from "$lib/config/uiFeatureFlags";
	import { commitUrl, FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { showToast } from "$lib/notifications/toasts";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE, withStackBusy } from "$lib/state/uiState.svelte";
	import { inject, injectOptional } from "@gitbutler/core/context";
	import { Button, TestId } from "@gitbutler/ui";
	import type { Commit, UpstreamCommit } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		laneId: string;
		commit: Commit | UpstreamCommit;
		active?: boolean;
		draggableFiles: boolean;
		grow?: boolean;
		clientHeight?: number;
		rounded?: boolean;
		ontoggle?: (collapsed: boolean) => void;
		onclose?: () => void;
		onpopout?: () => void;
	};

	let {
		projectId,
		stackId,
		laneId,
		commit,
		grow,
		clientHeight = $bindable(),
		rounded,
		ontoggle,
		onclose,
		onpopout,
	}: Props & { isInEditMessageMode?: boolean } = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const laneState = $derived(uiState.lane(laneId));
	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(laneState.selection);
	const branchName = $derived(selected.current?.branchName);

	const [updateCommitMessage, messageUpdateQuery] = stackService.updateCommitMessage;

	type Mode = "view" | "edit";

	function setMode(newMode: Mode) {
		switch (newMode) {
			case "edit":
				projectState.exclusiveAction.set({
					type: "edit-commit-message",
					stackId,
					branchName,
					commitId: commit.id,
				});
				break;
			case "view":
				projectState.exclusiveAction.set(undefined);
				break;
		}
	}

	const parsedMessage = $derived(splitMessage(commit.message));

	function combineParts(title?: string, description?: string): string {
		if (!title) {
			return "";
		}
		if (description) {
			return `${title}\n\n${description}`;
		}
		return title;
	}

	async function saveCommitMessage(title: string, description: string) {
		const commitMessage = combineParts(title, description);
		if (!branchName) {
			throw new Error("No branch selected!");
		}
		if (!commitMessage) {
			showToast({ message: "Commit message is required", style: "danger" });
			return;
		}

		const newCommitId = await updateCommitMessage({
			projectId,
			stackId,
			commitId: commit.id,
			message: commitMessage,
			dryRun: false,
		});

		if (stackId) {
			uiState.lane(stackId).selection.set({ branchName, commitId: newCommitId, previewOpen: true });
		}
		setMode("view");
	}

	async function handleUncommit() {
		if (!branchName) return;
		await withStackBusy(
			uiState,
			projectId,
			{ commitId: commit.id, stackIds: stackId ? [stackId] : undefined },
			async () => {
				await stackService.uncommit({
					projectId,
					stackId,
					commitIds: [commit.id],
				});
			},
		);
	}

	function canEdit() {
		return modeService !== undefined && !isReadOnly;
	}

	let drawer = $state<ReturnType<typeof Drawer>>();

	function cancelEdit() {
		setMode("view");
	}
</script>

<Drawer
	bind:this={drawer}
	bind:clientHeight
	testId={TestId.CommitDrawer}
	persistId="commit-view-drawer-{projectId}-{stackId}-{commit.id}"
	{grow}
	{rounded}
	{ontoggle}
	onclose={() => {
		// When the commit view is closed, we also want to unset the
		// relevant uiState so things like commit buttons are usable.

		// TODO: We should consider having a modal to confirm the cancel
		cancelEdit();
		onclose?.();
	}}
	bottomBorder={false}
	noshrink
>
	{#snippet closeActions()}
		{#if onpopout}
			<Button
				kind="ghost"
				icon="pop-out-bottom-right"
				size="tag"
				tooltip="Pop out diff view"
				onclick={onpopout}
			/>
		{/if}
	{/snippet}
	{#snippet header()}
		<CommitTitle
			truncate
			commitMessage={commit.message}
			className="text-14 text-semibold text-body"
			editable={!isReadOnly}
		/>
	{/snippet}

	{#snippet actions()}
		{#if canEdit()}
			{@const isEditingMessage =
				projectState.exclusiveAction.current?.type === "edit-commit-message" &&
				projectState.exclusiveAction.current.commitId === commit.id}
			<Button
				testId={TestId.CommitDrawerActionEditMessage}
				size="tag"
				kind="ghost"
				icon="edit"
				onclick={() => {
					drawer?.open();
					setMode("edit");
				}}
				tooltip={isReadOnly ? "Read-only mode" : "Reword commit"}
				disabled={isReadOnly || isEditingMessage}
			/>
		{/if}
		{@const data = isLocalAndRemoteCommit(commit)
			? {
					stackId,
					commitId: commit.id,
					commitMessage: commit.message,
					commitStatus: commit.state.type,
					commitUrl: forgeInfo ? commitUrl(forgeInfo, commit.id) : undefined,
					onUncommitClick: () => handleUncommit(),
					onEditMessageClick: () => {
						drawer?.open();
						setMode("edit");
					},
				}
			: undefined}
		{#if data}
			<CommitContextMenu {projectId} contextData={data} />
		{/if}
	{/snippet}

	<div class="commit-view">
		{#if projectState.exclusiveAction.current?.type === "edit-commit-message" && projectState.exclusiveAction.current.commitId === commit.id}
			<div
				class="edit-commit-view"
				data-testid={TestId.EditCommitMessageBox}
				class:no-paddings={uiState.global.useFloatingBox.current}
			>
				<CommitMessageEditor
					noPadding
					{projectId}
					{stackId}
					action={({ title, description }) => saveCommitMessage(title, description)}
					actionLabel="Save changes"
					onCancel={cancelEdit}
					floatingBoxHeader="Reword commit"
					loading={messageUpdateQuery.current.isLoading}
					existingCommitId={commit.id}
					title={parsedMessage?.title || ""}
					description={parsedMessage?.description || ""}
				/>
			</div>
		{:else}
			<CommitDetails {commit} rewrap={$rewrapCommitMessage} />
		{/if}
	</div>
</Drawer>

<style>
	.commit-view {
		position: relative;
		/* Limit the commit view to at most 40vh to ensure other sections remain visible */
		max-height: 50vh;
		background-color: var(--bg-1);
	}

	.edit-commit-view {
		display: flex;
		flex-direction: column;
		padding: 14px;

		&.no-paddings {
			margin: -14px;
		}
	}
</style>
