<script lang="ts" module>
	import type { Segment } from "@gitbutler/but-sdk";

	export type BranchHeaderContextData = {
		segment: Segment;
		prNumber?: number;
		first?: boolean;
		stackLength: number;
	};
</script>

<script lang="ts">
	import BranchRenameModal, {
		type BranchRenameModalProps,
	} from "$components/branch/BranchRenameModal.svelte";
	import DeleteBranchModal, {
		type DeleteBranchModalProps,
	} from "$components/branch/DeleteBranchModal.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { PROMPT_SERVICE } from "$lib/ai/aiPromptService";
	import { AI_SERVICE } from "$lib/ai/service";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { projectAiGenEnabled } from "$lib/config/config";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
		TestId,
	} from "@gitbutler/ui";

	import { tick } from "svelte";
	import type { AnchorPosition } from "$lib/stacks/stack";

	type Props = {
		projectId: string;
		stackId?: string;
		laneId: string;
		openId?: string;
		rightClickTrigger?: HTMLElement;
		contextData: BranchHeaderContextData;
	};

	let {
		projectId,
		stackId,
		laneId,
		openId = $bindable(),
		rightClickTrigger,
		contextData,
	}: Props = $props();

	const aiService = inject(AI_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const promptService = inject(PROMPT_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit.useMutation();
	const [updateBranchNameMutation] = stackService.updateBranchName;
	const [createRef, refCreation] = stackService.createReference;

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const branchName = $derived(contextData.segment.refName?.displayName);
	const branchReference = $derived(
		contextData.segment.refName
			? new TextDecoder().decode(new Uint8Array(contextData.segment.refName.fullNameBytes))
			: undefined,
	);
	const remoteTrackingBranch = $derived(contextData.segment.remoteTrackingRefName);
	const branchCommits = $derived(contextData.segment.commits);

	const branchType = $derived(branchCommits.at(0)?.state.type || "LocalOnly");
	const isConflicted = $derived(branchCommits.some((commit) => commit.hasConflicts));
	const hasCommits = $derived(branchCommits.length > 0);

	let aiConfigurationValid = $state(false);

	let renameBranchModal = $state<BranchRenameModal>();
	let renameBranchModalContext = $state<BranchRenameModalProps>();
	let deleteBranchModal = $state<DeleteBranchModal>();
	let deleteBranchModalContext = $state<DeleteBranchModalProps>();

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	async function generateBranchName(stackId: string, branchName: string) {
		if (!$aiGenEnabled || !aiConfigurationValid) return;

		const commitMessages = branchCommits.map((commit) => commit.message);
		// The context-menu entry is disabled via `hasCommits` when there are
		// no commits yet. Guard defensively against an empty list — silently
		// no-op rather than raising an error toast.
		if (commitMessages.length === 0) return;

		const prompt = promptService.selectedBranchPrompt(projectId);
		const newBranchName = await aiService.summarizeBranch({
			type: "commitMessages",
			commitMessages,
			branchTemplate: prompt,
		});

		if (newBranchName && newBranchName !== branchName) {
			await updateBranchNameMutation({
				projectId: projectId,
				stackId,
				laneId,
				branchName,
				newName: newBranchName,
			});
		}
	}

	async function handleCreateNewRef(stackId: string, position: AnchorPosition) {
		if (!branchName) return;
		const newName = await stackService.fetchNewBranchName(projectId);
		await createRef({
			projectId,
			stackId,
			request: {
				newName,
				anchor: {
					type: "atReference",
					subject: {
						short_name: branchName,
						position,
					},
				},
			},
		});
	}

	$effect(() => {
		setAIConfigurationValid();
	});
</script>

<KebabButton
	contextElement={rightClickTrigger}
	testId={TestId.KebabMenuButton}
	contextMenuTestId={TestId.BranchHeaderContextMenu}
>
	{#snippet contextMenu({ close })}
		{@const { prNumber, first, stackLength } = contextData}
		<ContextMenuSection>
			{#if remoteTrackingBranch && branchName}
				<ContextMenuItem
					label="Open in browser"
					icon="open-in-browser"
					testId={TestId.BranchHeaderContextMenu_OpenInBrowser}
					onclick={() => {
						const url = forge.current.branch(branchName)?.url;
						if (url) urlService.openExternalUrl(url);
						close();
					}}
				/>
			{/if}
			<ContextMenuItem
				label="Copy branch name"
				icon="copy"
				testId={TestId.BranchHeaderContextMenu_CopyBranchName}
				onclick={() => {
					if (branchName) {
						clipboardService.write(branchName, { message: "Branch name copied" });
					}
					close();
				}}
				disabled={!branchName}
			/>
		</ContextMenuSection>
		{#if stackId}
			<ContextMenuSection>
				<ContextMenuItemSubmenu
					label="Create branch"
					icon="stack-plus"
					disabled={isReadOnly || refCreation.current.isLoading}
				>
					{#snippet submenu({ close: closeSubmenu })}
						<ContextMenuSection>
							<ContextMenuItem
								label="Create branch above"
								testId={TestId.BranchHeaderContextMenu_AddDependentBranch}
								disabled={isReadOnly}
								onclick={async () => {
									await handleCreateNewRef(stackId, "Above");
									closeSubmenu();
									close();
								}}
							/>
							<ContextMenuItem
								label="Create branch below"
								disabled={isReadOnly}
								onclick={async () => {
									await handleCreateNewRef(stackId, "Below");
									closeSubmenu();
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</ContextMenuItemSubmenu>
				<ContextMenuItem
					label="Add empty commit"
					icon="commit-plus"
					testId={TestId.BranchHeaderContextMenu_AddEmptyCommit}
					onclick={async () => {
						if (!branchReference) return;
						await insertBlankCommitInBranch({
							projectId,
							relativeTo: { type: "reference", subject: branchReference },
							side: "below",
							dryRun: false,
						});
						close();
					}}
					disabled={isReadOnly || commitInsertion.current.isLoading || !branchReference}
				/>
				{#if branchCommits.length > 1 && branchName}
					<ContextMenuItem
						label="Squash all commits"
						icon="commit-double-chevron-down"
						testId={TestId.BranchHeaderContextMenu_SquashAllCommits}
						onclick={async () => {
							await stackService.squashAllCommits({
								projectId,
								stackId,
								branchName,
							});
							close();
						}}
						disabled={isReadOnly || isConflicted}
					/>
				{/if}
			</ContextMenuSection>
			<ContextMenuSection>
				{#if $aiGenEnabled && aiConfigurationValid && !remoteTrackingBranch && stackId && branchName}
					<ContextMenuItem
						label="Generate branch name"
						icon="edit-ai"
						testId={TestId.BranchHeaderContextMenu_GenerateBranchName}
						disabled={isReadOnly || !hasCommits}
						onclick={() => {
							generateBranchName(stackId, branchName);
							close();
						}}
					/>
				{/if}
				{#if branchType !== "Integrated" && branchName}
					<ContextMenuItem
						label="Rename"
						icon="edit"
						testId={TestId.BranchHeaderContextMenu_Rename}
						disabled={isReadOnly}
						onclick={async () => {
							renameBranchModalContext = {
								projectId,
								stackId,
								laneId,
								branchName,
								isPushed: !!remoteTrackingBranch,
							};
							await tick();
							renameBranchModal?.show();
							close();
						}}
					/>
				{/if}
				{#if branchName && stackLength && ((stackLength > 1 && (!first || !hasCommits)) || (stackLength === 1 && branchCommits.length === 0))}
					<ContextMenuItem
						label="Delete"
						icon="bin"
						testId={TestId.BranchHeaderContextMenu_Delete}
						disabled={isReadOnly}
						onclick={async () => {
							deleteBranchModalContext = {
								projectId,
								stackId,
								branchName,
							};
							await tick();
							deleteBranchModal?.show();
							close();
						}}
					/>
				{/if}
			</ContextMenuSection>
		{/if}
		{#if prNumber}
			{@const prQuery = forge.current.prService?.get(prNumber)}
			<ReduxResult {projectId} {stackId} result={prQuery?.result}>
				{#snippet children(pr)}
					<ContextMenuSection>
						<ContextMenuItemSubmenu label={forge.reviewUnitName} icon="pr">
							{#snippet submenu({ close: closeSubmenu })}
								<ContextMenuSection>
									<ContextMenuItem
										label="Open {forge.reviewUnitAbbr} in browser"
										testId={TestId.BranchHeaderContextMenu_OpenPRInBrowser}
										onclick={() => {
											urlService.openExternalUrl(pr.htmlUrl);
											closeSubmenu();
											close();
										}}
									/>
									<ContextMenuItem
										label="Copy {forge.reviewUnitAbbr} link"
										testId={TestId.BranchHeaderContextMenu_CopyPRLink}
										onclick={() => {
											clipboardService.write(pr.htmlUrl, {
												message: `${forge.reviewUnitAbbr} link copied`,
											});
											closeSubmenu();
											close();
										}}
									/>
								</ContextMenuSection>
							{/snippet}
						</ContextMenuItemSubmenu>
					</ContextMenuSection>
				{/snippet}
				<!-- For now, just swallow this error -->
				{#snippet error()}{/snippet}
			</ReduxResult>
		{/if}

		{#if stackId && first}
			<ContextMenuSection>
				<ContextMenuItem
					label="Unapply Stack"
					icon="eject"
					testId={TestId.BranchHeaderContextMenu_UnapplyBranch}
					disabled={isReadOnly}
					onclick={async () => {
						try {
							await stackService.unapply({ projectId, stackId });
						} finally {
							close();
						}
					}}
				/>
			</ContextMenuSection>
		{/if}
	{/snippet}
</KebabButton>

{#if renameBranchModalContext}
	<BranchRenameModal bind:this={renameBranchModal} {...renameBranchModalContext} />
{/if}

{#if deleteBranchModalContext}
	<DeleteBranchModal bind:this={deleteBranchModal} {...deleteBranchModalContext} />
{/if}
