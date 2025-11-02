<script lang="ts" module>
	export type BranchHeaderContextData = {
		branch: BranchDetails;
		prNumber?: number;
		first?: boolean;
		stackLength: number;
	};
</script>

<script lang="ts">
	import BranchRenameModal, {
		type BranchRenameModalProps
	} from '$components/BranchRenameModal.svelte';
	import DeleteBranchModal, {
		type DeleteBranchModalProps
	} from '$components/DeleteBranchModal.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { PROMPT_SERVICE } from '$lib/ai/promptService';
	import { AI_SERVICE } from '$lib/ai/service';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { useGoToCodegenPage } from '$lib/codegen/redirect.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import {
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
		TestId
	} from '@gitbutler/ui';

	import { tick } from 'svelte';
	import type { AnchorPosition, BranchDetails } from '$lib/stacks/stack';

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
		contextData
	}: Props = $props();

	const { goToCodegenPage } = useGoToCodegenPage();

	const aiService = inject(AI_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const promptService = inject(PROMPT_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;
	const [updateBranchNameMutation] = stackService.updateBranchName;
	const [createRef, refCreation] = stackService.createReference;

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const allCommits = $derived.by(() => {
		return stackId
			? stackService.commits(projectId, stackId, contextData.branch.name)
			: stackService.unstackedCommits(projectId, contextData.branch.name);
	});

	async function getAllCommits() {
		if (stackId) {
			return stackService.fetchCommits(projectId, stackId, contextData.branch.name);
		}
		return stackService.fetchUnstackedCommits(projectId, contextData.branch.name);
	}

	const commits = $derived(allCommits?.response);
	const branchType = $derived(commits?.at(0)?.state.type || 'LocalOnly');
	const isConflicted = $derived(commits?.some((commit) => commit.hasConflicts) ?? false);

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

		const commits = await getAllCommits();
		const commitMessages = commits?.map((commit) => commit.message) ?? [];
		if (commitMessages.length === 0) {
			throw new Error('There must be commits in the branch before you can generate a branch name');
		}

		const prompt = promptService.selectedBranchPrompt(projectId);
		const newBranchName = await aiService.summarizeBranch({
			type: 'commitMessages',
			commitMessages,
			branchTemplate: prompt
		});

		if (newBranchName && newBranchName !== branchName) {
			await updateBranchNameMutation({
				projectId: projectId,
				stackId,
				laneId,
				branchName,
				newName: newBranchName
			});
		}
	}

	async function handleCreateNewRef(stackId: string, position: AnchorPosition) {
		const newName = await stackService.fetchNewBranchName(projectId);
		await createRef({
			projectId,
			stackId,
			request: {
				newName,
				anchor: {
					type: 'atReference',
					subject: {
						short_name: contextData.branch.name,
						position
					}
				}
			}
		});
	}

	$effect(() => {
		setAIConfigurationValid();
	});
</script>

{#if rightClickTrigger}
	<KebabButton contextElement={rightClickTrigger} testId={TestId.KebabMenuButton}>
		{#snippet contextMenu({ close })}
			{@const { branch, prNumber, first, stackLength } = contextData}
			{@const branchName = branch.name}
			<ContextMenuSection>
				{#if branch.remoteTrackingBranch}
					<ContextMenuItem
						label="Open in browser"
						icon="open-link"
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
						clipboardService.write(branch?.name, { message: 'Branch name copied' });
						close();
					}}
				/>
			</ContextMenuSection>
			{#if stackId}
				<ContextMenuSection>
					<ContextMenuItemSubmenu
						label="Create branch"
						icon="new-dep-branch"
						disabled={isReadOnly || refCreation.current.isLoading}
					>
						{#snippet submenu({ close: closeSubmenu })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Create branch above"
									testId={TestId.BranchHeaderContextMenu_AddDependentBranch}
									disabled={isReadOnly}
									onclick={async () => {
										await handleCreateNewRef(stackId, 'Above');
										closeSubmenu();
										close();
									}}
								/>
								<ContextMenuItem
									label="Create branch below"
									disabled={isReadOnly}
									onclick={async () => {
										await handleCreateNewRef(stackId, 'Below');
										closeSubmenu();
										close();
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
					<ContextMenuItem
						label="Add empty commit"
						icon="new-empty-commit"
						testId={TestId.BranchHeaderContextMenu_AddEmptyCommit}
						onclick={async () => {
							await insertBlankCommitInBranch({
								projectId,
								stackId,
								commitId: undefined,
								offset: -1
							});
							close();
						}}
						disabled={isReadOnly || commitInsertion.current.isLoading}
					/>
					{#if branch.commits.length > 1}
						<ContextMenuItem
							label="Squash all commits"
							icon="squash-commits"
							testId={TestId.BranchHeaderContextMenu_SquashAllCommits}
							onclick={async () => {
								await stackService.squashAllCommits({
									projectId,
									stackId,
									branchName
								});
								close();
							}}
							disabled={isReadOnly || isConflicted}
							tooltip={isReadOnly
								? 'Read-only mode'
								: isConflicted
									? 'This branch has conflicts'
									: undefined}
						/>
					{/if}
				</ContextMenuSection>
				<ContextMenuSection>
					{#if stackId && first}
						{@const rule = rulesService.aiRuleForStack({ projectId, stackId })}
						{#if !rule.response}
							<ContextMenuItem
								label="Start agent session"
								icon="agents-tab"
								testId={TestId.BranchHeaderContextMenu_StartCodegenAgent}
								disabled={isReadOnly}
								onclick={() => {
									goToCodegenPage(projectId, stackId, branchName);
									close();
								}}
							/>
						{/if}
					{/if}
					{#if $aiGenEnabled && aiConfigurationValid && !branch.remoteTrackingBranch && stackId}
						<ContextMenuItem
							label="Generate branch name"
							icon="ai-edit"
							testId={TestId.BranchHeaderContextMenu_GenerateBranchName}
							disabled={isReadOnly}
							onclick={() => {
								generateBranchName(stackId, branchName);
								close();
							}}
						/>
					{/if}
					{#if branchType !== 'Integrated'}
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
									isPushed: !!branch.remoteTrackingBranch
								};
								await tick();
								renameBranchModal?.show();
								close();
							}}
						/>
					{/if}
					{#if stackLength && (stackLength > 1 || (stackLength === 1 && branch.commits.length === 0))}
						<ContextMenuItem
							label="Delete"
							icon="bin"
							testId={TestId.BranchHeaderContextMenu_Delete}
							disabled={isReadOnly}
							onclick={async () => {
								deleteBranchModalContext = {
									projectId,
									stackId,
									branchName
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
							<ContextMenuItemSubmenu label="Pull Request" icon="pr">
								{#snippet submenu({ close: closeSubmenu })}
									<ContextMenuSection>
										<ContextMenuItem
											label="Open PR in browser"
											testId={TestId.BranchHeaderContextMenu_OpenPRInBrowser}
											onclick={() => {
												urlService.openExternalUrl(pr.htmlUrl);
												closeSubmenu();
												close();
											}}
										/>
										<ContextMenuItem
											label="Copy PR link"
											testId={TestId.BranchHeaderContextMenu_CopyPRLink}
											onclick={() => {
												clipboardService.write(pr.htmlUrl, { message: 'PR link copied' });
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
						disabled={isReadOnly}
						onclick={async () => {
							await stackService.unapply({ projectId, stackId });
							close();
						}}
					/>
				</ContextMenuSection>
			{/if}
		{/snippet}
	</KebabButton>
{/if}

{#if renameBranchModalContext}
	<BranchRenameModal bind:this={renameBranchModal} {...renameBranchModalContext} />
{/if}

{#if deleteBranchModalContext}
	<DeleteBranchModal bind:this={deleteBranchModal} {...deleteBranchModalContext} />
{/if}
