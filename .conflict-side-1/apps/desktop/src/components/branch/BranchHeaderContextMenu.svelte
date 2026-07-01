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
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { projectAiGenEnabled } from "$lib/config/config";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
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
	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const promptService = inject(PROMPT_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const reviewUnitName = $derived(forgeInfo?.unit.name ?? "Pull request");
	const reviewUnitAbbr = $derived(forgeInfo?.unit.abbr ?? "PR");
	const baseBranchNameQuery = $derived(baseBranchService.baseBranchShortName(projectId));
	const baseBranchName = $derived(baseBranchNameQuery.response);
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
	// Fork namespace for cross-fork compare URLs (GitHub `owner:branch`),
	// set only when the branch is pushed to a different repo than the base.
	const repoQuery = $derived(baseBranchService.repo(projectId));
	const pushRepoQuery = $derived(baseBranchService.pushRepo(projectId));
	const fork = $derived.by(() => {
		const repo = repoQuery.response;
		const pushRepo = pushRepoQuery.response;
		return pushRepo && repo && pushRepo.hash !== repo.hash ? pushRepo.owner : null;
	});
	const compareBranchUrlQuery = $derived(
		branchName && baseBranchName
			? forgeInfoService.compareBranchUrl(projectId, baseBranchName, branchName, fork)
			: undefined,
	);
	const branchUrl = $derived(compareBranchUrlQuery?.response);

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
					type: "atSegment",
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
						if (branchUrl) urlService.openExternalUrl(branchUrl);
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
			{@const prQuery = prService.get(projectId, prNumber)}
			<ReduxResult {projectId} {stackId} result={prQuery.result}>
				{#snippet children(pr)}
					<ContextMenuSection>
						<ContextMenuItemSubmenu label={reviewUnitName} icon="pr">
							{#snippet submenu({ close: closeSubmenu })}
								<ContextMenuSection>
									<ContextMenuItem
										label="Open {reviewUnitAbbr} in browser"
										testId={TestId.BranchHeaderContextMenu_OpenPRInBrowser}
										onclick={() => {
											urlService.openExternalUrl(pr.htmlUrl);
											closeSubmenu();
											close();
										}}
									/>
									<ContextMenuItem
										label="Copy {reviewUnitAbbr} link"
										testId={TestId.BranchHeaderContextMenu_CopyPRLink}
										onclick={() => {
											clipboardService.write(pr.htmlUrl, {
												message: `${reviewUnitAbbr} link copied`,
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
