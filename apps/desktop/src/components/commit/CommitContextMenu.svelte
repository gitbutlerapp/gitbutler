<script lang="ts" module>
	import type { CommitStatusType } from "$lib/commits/commit";
	interface BaseContextData {
		commitStatus: CommitStatusType;
		commitId: string;
		commitMessage: string;
		commitUrl?: string;
	}

	interface LocalCommitContextData extends BaseContextData {
		commitStatus: "LocalOnly" | "LocalAndRemote";
		stackId?: string;
		hasConflicts?: boolean;
		onUncommitClick: (event: MouseEvent) => void;
		onEditMessageClick: (event: MouseEvent) => void;
		/** When set, indicates multiple commits are selected. */
		multiSelect?: {
			commitIds: string[];
			onSquashSelected: () => void;
			onUncommitSelected: () => void;
		};
	}

	interface RemoteCommitContextData extends BaseContextData {
		commitStatus: "Remote";
		stackId?: string;
	}

	interface IntegratedCommitContextData extends BaseContextData {
		commitStatus: "Integrated";
		stackId?: string;
	}

	interface BaseCommitContextData extends BaseContextData {
		commitStatus: "Base";
	}

	export type CommitContextData =
		| LocalCommitContextData
		| RemoteCommitContextData
		| IntegratedCommitContextData
		| BaseCommitContextData;

	export type CommitMenuContext = {
		position: { coords?: { x: number; y: number }; element?: HTMLElement };
		data: CommitContextData;
	};

	// Commits with an AI resolution in flight. Module-scoped because the menu
	// instance (and its mutation state) is destroyed when the menu closes,
	// while the resolution keeps running.
	const resolvingCommits = new Set<string>();
</script>

<script lang="ts">
	import IrcSendToSubmenus from "$components/diff/IrcSendToSubmenus.svelte";
	import { AI_SERVICE } from "$lib/ai/service";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { projectAiGenEnabled } from "$lib/config/config";
	import { rewrapCommitMessage } from "$lib/config/uiFeatureFlags";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { Messages, serialize } from "$lib/irc/protocol";
	import { buildSharedCommitPayload } from "$lib/irc/sharedStack";
	import { editPatch } from "$lib/mode/editPatchUtils";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { dismissToast, showToast } from "$lib/notifications/toasts";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject, injectOptional } from "@gitbutler/core/context";
	import {
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
		TestId,
	} from "@gitbutler/ui";
	import type { AnchorPosition } from "$lib/stacks/stack";

	type Props = {
		showOnHover?: boolean;
		projectId: string;
		openId?: string;
		rightClickTrigger?: HTMLElement;
		contextData: CommitContextData | undefined;
	};

	let {
		showOnHover,
		projectId,
		openId = $bindable(),
		rightClickTrigger,
		contextData,
	}: Props = $props();

	const urlService = inject(URL_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const diffService = inject(DIFF_SERVICE);
	const ircApiService = inject(IRC_API_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const aiService = inject(AI_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit.useMutation();
	const [createRef, refCreation] = stackService.createReference;
	const [resolveConflictsAi, aiResolution] = stackService.resolveCommitConflictsAi;

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const commitHasConflicts = $derived(
		contextData !== undefined && "hasConflicts" in contextData && !!contextData.hasConflicts,
	);
	let aiConfigurationValid = $state(false);

	// Validating the AI configuration costs several backend calls, so only do
	// it for the rare rows that can actually offer the AI-resolve action.
	$effect(() => {
		if (!commitHasConflicts || !$aiGenEnabled) return;
		let stale = false;
		aiService.validateConfiguration().then(
			(valid) => {
				if (!stale) aiConfigurationValid = valid;
			},
			() => {
				if (!stale) aiConfigurationValid = false;
			},
		);
		return () => {
			stale = true;
		};
	});

	const projectQuery = $derived(projectsService.getProject(projectId));
	const projectTitle = $derived(projectQuery.response?.title ?? projectId);

	let sending = $state(false);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(
		contextData?.commitStatus === "LocalAndRemote" || contextData?.commitStatus === "LocalOnly"
			? !contextData.stackId
			: false,
	);

	async function insertBlankCommit(commitId: string, location: "above" | "below" = "below") {
		await insertBlankCommitInBranch({
			projectId,
			relativeTo: { type: "commit", subject: commitId },
			side: location,
			dryRun: false,
		});
	}

	async function handleCreateNewRef(
		stackId: string | undefined,
		commitId: string,
		position: AnchorPosition,
	) {
		const newName = await stackService.fetchNewBranchName(projectId);
		await createRef({
			projectId,
			stackId,
			request: {
				newName,
				anchor: {
					type: "atCommit",
					subject: {
						commit_id: commitId,
						position,
					},
				},
			},
		});
	}

	async function handleEditPatch(commitId: string, stackId: string) {
		if (isReadOnly) return;
		await editPatch({
			modeService,
			commitId,
			stackId,
			projectId,
		});
	}

	async function handleResolveConflictsAi(commitId: string, stackId: string) {
		if (isReadOnly || !$aiGenEnabled || !aiConfigurationValid) return;
		if (resolvingCommits.has(commitId)) return;
		resolvingCommits.add(commitId);
		const progressToastId = `resolve-conflicts-ai-${commitId}`;
		showToast({
			id: progressToastId,
			style: "info",
			title: "Resolving conflicts with AI…",
			message: "This can take a moment. The resolution is applied when it completes.",
		});
		try {
			const result = await resolveConflictsAi({ projectId, stackId, commitId });
			dismissToast(progressToastId);
			const fileList = result.files
				.map((file) => `- \`${file.path}\` — ${file.reasoning}`)
				.join("\n");
			showToast({
				style: "success",
				title: "Conflicts resolved with AI",
				message: `${result.summary ?? ""}\n\n${fileList}\n\nIf this isn't right, undo it from the operations history.`,
			});
		} catch (error: unknown) {
			dismissToast(progressToastId);
			showToast({
				style: "danger",
				title: "Failed to resolve conflicts with AI",
				error,
			});
		} finally {
			resolvingCommits.delete(commitId);
		}
	}

	async function sendCommitToChannel(
		channelName: string,
		commitId: string,
		commitMessage: string,
		stackId: string,
	) {
		if (sending) return;
		sending = true;
		try {
			const payload = await buildSharedCommitPayload(
				stackId,
				commitId,
				projectId,
				projectTitle,
				stackService,
				diffService,
			);
			const msg = Messages.sharedCommit({ sender: "me", commit: payload });
			const { text, data } = serialize(msg);
			await ircApiService.sendMessageWithData({
				target: channelName,
				message: text,
				data,
			});
		} finally {
			sending = false;
		}
	}
</script>

{#if contextData}
	<KebabButton
		{showOnHover}
		contextElement={rightClickTrigger}
		testId={TestId.KebabMenuButton}
		contextMenuTestId={TestId.CommitRowContextMenu}
	>
		{#snippet contextMenu({ close })}
			{@const { commitId, commitUrl, commitMessage } = contextData}
			{@const isLocal =
				contextData.commitStatus === "LocalAndRemote" || contextData.commitStatus === "LocalOnly"}
			{@const multiSelect = isLocal ? contextData.multiSelect : undefined}
			{@const isMultiSelect = multiSelect && multiSelect.commitIds.length > 1}

			{#if isLocal}
				{#if isMultiSelect}
					<!-- Multi-select actions -->
					<ContextMenuSection>
						<ContextMenuItem
							label="Squash {multiSelect.commitIds.length} commits"
							icon="commit-double-chevron-down"
							testId={TestId.CommitRowContextMenu_SquashSelected}
							disabled={isReadOnly}
							onclick={() => {
								if (!isReadOnly) {
									multiSelect.onSquashSelected();
									close();
								}
							}}
						/>
						<ContextMenuItem
							label="Uncommit {multiSelect.commitIds.length} commits"
							icon="undo"
							testId={TestId.CommitRowContextMenu_UncommitSelected}
							disabled={isReadOnly}
							onclick={() => {
								if (!isReadOnly) {
									multiSelect.onUncommitSelected();
									close();
								}
							}}
						/>
					</ContextMenuSection>
				{:else}
					<!-- Single-commit actions -->
					{@const { onUncommitClick, onEditMessageClick } = contextData}
					<ContextMenuSection>
						<ContextMenuItem
							label="Uncommit"
							icon="undo"
							testId={TestId.CommitRowContextMenu_UncommitMenuButton}
							disabled={isReadOnly}
							onclick={(e: MouseEvent) => {
								if (!isReadOnly) {
									onUncommitClick?.(e);
									close();
								}
							}}
						/>
						<ContextMenuItem
							label="Reword commit"
							icon="edit"
							testId={TestId.CommitRowContextMenu_EditMessageMenuButton}
							disabled={isReadOnly}
							onclick={(e: MouseEvent) => {
								if (!isReadOnly) {
									onEditMessageClick?.(e);
									close();
								}
							}}
						/>
						<ContextMenuItem
							label="Edit commit"
							icon="commit-edit"
							testId={TestId.CommitRowContextMenu_EditCommit}
							disabled={isReadOnly}
							onclick={async () => {
								if (!isReadOnly && contextData.stackId) {
									await handleEditPatch(commitId, contextData.stackId);
									close();
								}
							}}
						/>
						{#if contextData.hasConflicts && $aiGenEnabled && aiConfigurationValid}
							<ContextMenuItem
								label="Resolve conflicts with AI"
								icon="ai"
								testId={TestId.CommitRowContextMenu_ResolveConflictsAi}
								disabled={isReadOnly || aiResolution.current.isLoading}
								onclick={() => {
									if (!isReadOnly && contextData.stackId) {
										handleResolveConflictsAi(commitId, contextData.stackId);
										close();
									}
								}}
							/>
						{/if}
					</ContextMenuSection>
				{/if}
			{/if}

			{#if !isMultiSelect}
				<ContextMenuSection>
					{#if commitUrl}
						<ContextMenuItem
							label="Open in browser"
							icon="open-in-browser"
							onclick={async () => {
								await urlService.openExternalUrl(commitUrl);
								close();
							}}
						/>
					{/if}
					<ContextMenuItemSubmenu label="Copy" icon="copy">
						{#snippet submenu({ close: closeSubmenu })}
							<ContextMenuSection>
								{#if commitUrl}
									<ContextMenuItem
										label="Copy commit link"
										onclick={() => {
											clipboardService.write(commitUrl, { message: "Commit link copied" });
											closeSubmenu();
											close();
										}}
									/>
								{/if}
								<ContextMenuItem
									label="Copy commit hash"
									onclick={() => {
										clipboardService.write(commitId, { message: "Commit hash copied" });
										closeSubmenu();
										close();
									}}
								/>
								<ContextMenuItem
									label="Copy commit message"
									onclick={() => {
										clipboardService.write(commitMessage, { message: "Commit message copied" });
										closeSubmenu();
										close();
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
					{#if isLocal}
						{@const stackId = contextData.stackId}

						<ContextMenuItemSubmenu label="Add empty commit" icon="commit-plus">
							{#snippet submenu({ close: closeSubmenu })}
								<ContextMenuSection>
									<ContextMenuItem
										label="Add empty commit above"
										disabled={isReadOnly || commitInsertion.current.isLoading}
										onclick={() => {
											insertBlankCommit(commitId, "above");
											closeSubmenu();
											close();
										}}
									/>
									<ContextMenuItem
										label="Add empty commit below"
										disabled={isReadOnly || commitInsertion.current.isLoading}
										onclick={() => {
											insertBlankCommit(commitId, "below");
											closeSubmenu();
											close();
										}}
									/>
								</ContextMenuSection>
							{/snippet}
						</ContextMenuItemSubmenu>
						<ContextMenuItemSubmenu label="Create branch" icon="branch">
							{#snippet submenu({ close: closeSubmenu })}
								<ContextMenuSection>
									<ContextMenuItem
										label="Add branch above"
										disabled={isReadOnly || refCreation.current.isLoading}
										onclick={async () => {
											if (!isReadOnly) {
												await handleCreateNewRef(stackId, commitId, "Above");
												closeSubmenu();
												close();
											}
										}}
									/>
									<ContextMenuItem
										label="Add branch below"
										disabled={isReadOnly || refCreation.current.isLoading}
										onclick={async () => {
											if (!isReadOnly) {
												await handleCreateNewRef(stackId, commitId, "Below");
												closeSubmenu();
												close();
											}
										}}
									/>
								</ContextMenuSection>
							{/snippet}
						</ContextMenuItemSubmenu>
					{/if}
				</ContextMenuSection>

				{#if "stackId" in contextData && contextData.stackId}
					{@const ctxStackId = contextData.stackId}
					<IrcSendToSubmenus
						{projectId}
						disabled={sending}
						onSend={(target) => sendCommitToChannel(target, commitId, commitMessage, ctxStackId)}
						closeMenu={close}
					/>
				{/if}

				<ContextMenuSection>
					<ContextMenuItem
						label={$rewrapCommitMessage ? "Show original wrapping" : "Rewrap message"}
						icon="text-wrap"
						disabled={commitInsertion.current.isLoading}
						onclick={() => {
							rewrapCommitMessage.set(!$rewrapCommitMessage);
							close();
						}}
					/>
				</ContextMenuSection>
			{/if}
		{/snippet}
	</KebabButton>
{/if}
