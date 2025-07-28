<script lang="ts" module>
	export type BranchHeaderContextItem = {
		data: {
			branch: BranchDetails;
			prNumber?: number;
			first?: boolean;
			stackLength: number;
		};
		position: { coords?: { x: number; y: number }; element?: HTMLElement };
	};
</script>

<script lang="ts">
	import ContextMenu from '$components/shared/ContextMenu.svelte';
	import ReduxResult from '$components/shared/ReduxResult.svelte';
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps
	} from '$components/shared/branches/AddDependentBranchModal.svelte';
	import BranchRenameModal, {
		type BranchRenameModalProps
	} from '$components/shared/branches/BranchRenameModal.svelte';
	import DeleteBranchModal, {
		type DeleteBranchModalProps
	} from '$components/shared/branches/DeleteBranchModal.svelte';
	import { PROMPT_SERVICE } from '$lib/ai/promptService';
	import { AI_SERVICE } from '$lib/ai/service';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import { ContextMenuItem, ContextMenuSection } from '@gitbutler/ui';

	import { tick } from 'svelte';
	import type { BranchDetails } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId?: string;
		openId?: string;
		context?: BranchHeaderContextItem;
	};

	let { projectId, stackId, context = $bindable(), openId: openId = $bindable() }: Props = $props();

	const aiService = inject(AI_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const promptService = inject(PROMPT_SERVICE);
	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;
	const [updateBranchNameMutation] = stackService.updateBranchName;

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const allCommits = $derived.by(() => {
		if (!context) return;
		return stackId
			? stackService.commits(projectId, stackId, context.data.branch.name)
			: stackService.unstackedCommits(projectId, context.data.branch.name);
	});

	const commits = $derived(allCommits?.current.data);
	const branchType = $derived(commits?.at(0)?.state.type || 'LocalOnly');
	const isConflicted = $derived(commits?.some((commit) => commit.hasConflicts) ?? false);

	let aiConfigurationValid = $state(false);

	let contextMenu = $state<ContextMenu>();

	let renameBranchModal = $state<BranchRenameModal>();
	let renameBranchModalContext = $state<BranchRenameModalProps>();
	let deleteBranchModal = $state<DeleteBranchModal>();
	let deleteBranchModalContext = $state<DeleteBranchModalProps>();
	let addDependentBranchModal = $state<AddDependentBranchModal>();
	let addDependentBranchModalContext = $state<AddDependentBranchModalProps>();

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	async function generateBranchName(stackId: string, branchName: string) {
		if (!$aiGenEnabled || !aiConfigurationValid) return;

		const commitMessages = commits?.map((commit) => commit.message) ?? [];
		if (commitMessages.length === 0) {
			throw new Error(
				'There must be a commits in the branch before you can generate a branch name'
			);
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
				branchName,
				newName: newBranchName
			});
		}
	}

	$effect(() => {
		setAIConfigurationValid();
	});

	function close() {
		context = undefined;
	}
</script>

{#if context}
	{@const { branch, prNumber, first, stackLength } = context.data}
	{@const branchName = branch.name}
	<ContextMenu
		bind:this={contextMenu}
		onclose={() => (context = undefined)}
		testId={TestId.BranchHeaderContextMenu}
		position={context.position}
	>
		{#if first && stackId}
			<ContextMenuSection>
				<ContextMenuItem
					label="Add dependent branch"
					testId={TestId.BranchHeaderContextMenu_AddDependentBranch}
					onclick={async () => {
						addDependentBranchModalContext = {
							projectId,
							stackId
						};

						await tick();

						addDependentBranchModal?.show();
						close();
					}}
				/>
			</ContextMenuSection>
		{/if}
		<ContextMenuSection>
			{#if branch.remoteTrackingBranch}
				<ContextMenuItem
					label="Open in browser"
					testId={TestId.BranchHeaderContextMenu_OpenInBrowser}
					onclick={() => {
						const url = forge.current.branch(branchName)?.url;
						if (url) openExternalUrl(url);
						close();
					}}
				/>
			{/if}
			<ContextMenuItem
				label="Copy branch name"
				testId={TestId.BranchHeaderContextMenu_CopyBranchName}
				onclick={() => {
					writeClipboard(branch?.name);
					close();
				}}
			/>
		</ContextMenuSection>
		{#if stackId}
			<ContextMenuSection>
				<ContextMenuItem
					label="Add empty commit"
					onclick={async () => {
						await insertBlankCommitInBranch({
							projectId,
							stackId,
							commitId: undefined,
							offset: -1
						});
						close();
					}}
					disabled={commitInsertion.current.isLoading}
				/>
				{#if branch.commits.length > 1}
					<ContextMenuItem
						label="Squash all commits"
						testId={TestId.BranchHeaderContextMenu_SquashAllCommits}
						onclick={async () => {
							await stackService.squashAllCommits({
								projectId,
								stackId,
								branchName
							});
							close();
						}}
						disabled={isConflicted}
						tooltip={isConflicted ? 'This branch has conflicts' : undefined}
					/>
				{/if}
				{#if $aiGenEnabled && aiConfigurationValid && !branch.remoteTrackingBranch && stackId}
					<ContextMenuItem
						label="Generate branch name"
						testId={TestId.BranchHeaderContextMenu_GenerateBranchName}
						onclick={() => {
							generateBranchName(stackId, branchName);
							close();
						}}
					/>
				{/if}
				{#if branchType !== 'Integrated'}
					<ContextMenuItem
						label="Rename"
						testId={TestId.BranchHeaderContextMenu_Rename}
						onclick={async () => {
							renameBranchModalContext = {
								projectId,
								stackId,
								branchName,
								isPushed: !!branch.remoteTrackingBranch
							};
							await tick();
							renameBranchModal?.show();
							close();
						}}
					/>
				{/if}
				{#if stackLength && stackLength > 1}
					<ContextMenuItem
						label="Delete"
						testId={TestId.BranchHeaderContextMenu_Delete}
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
			{@const prResult = forge.current.prService?.get(prNumber)}
			<ReduxResult {projectId} {stackId} result={prResult?.current}>
				{#snippet children(pr)}
					<ContextMenuSection>
						<ContextMenuItem
							label="Open PR in browser"
							testId={TestId.BranchHeaderContextMenu_OpenPRInBrowser}
							onclick={() => {
								openExternalUrl(pr.htmlUrl);
								close();
							}}
						/>
						<ContextMenuItem
							label="Copy PR link"
							testId={TestId.BranchHeaderContextMenu_CopyPRLink}
							onclick={() => {
								writeClipboard(pr.htmlUrl);
								close();
							}}
						/>
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
					onclick={async () => {
						await stackService.unapply({ projectId, stackId });
						close();
					}}
				/>
			</ContextMenuSection>
		{/if}
	</ContextMenu>
{/if}

{#if renameBranchModalContext}
	<BranchRenameModal bind:this={renameBranchModal} {...renameBranchModalContext} />
{/if}

{#if deleteBranchModalContext}
	<DeleteBranchModal bind:this={deleteBranchModal} {...deleteBranchModalContext} />
{/if}

{#if addDependentBranchModalContext}
	<AddDependentBranchModal
		bind:this={addDependentBranchModal}
		{...addDependentBranchModalContext}
	/>
{/if}
