<script lang="ts" module>
	export type BranchHeaderContextItem = {
		data: {
			branch: BranchDetails;
			prNumber?: number;
			first?: boolean;
			stackLength?: number;
		};
		position: { coords?: { x: number; y: number }; element?: HTMLElement };
	};
</script>

<script lang="ts">
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import DeleteBranchModal from '$components/DeleteBranchModal.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import ContextMenu from '$components/v3/ContextMenu.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { BranchDetails } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId?: string;
		openId?: string;
		context?: BranchHeaderContextItem;
	};

	let { projectId, stackId, context = $bindable(), openId: openId = $bindable() }: Props = $props();

	const [aiService, stackService, forge, promptService] = inject(
		AIService,
		StackService,
		DefaultForgeFactory,
		PromptService
	);
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
	let deleteBranchModal = $state<DeleteBranchModal>();

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	async function generateBranchName(stackId: string, branchName: string) {
		if (!$aiGenEnabled || !aiConfigurationValid) return;

		const commitMessages = commits?.map((commit) => commit.message) ?? [];
		const prompt = promptService.selectedBranchPrompt(projectId);
		const newBranchName = await aiService.summarizeBranch({
			type: 'commitMessages',
			commitMessages,
			branchTemplate: prompt
		});

		if (newBranchName && newBranchName !== branchName) {
			updateBranchNameMutation({
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
		{#if first}
			<ContextMenuSection>
				<ContextMenuItem
					label="Add dependent branch"
					testId={TestId.BranchHeaderContextMenu_AddDependentBranch}
					onclick={() => {
						// onAddDependentSeries?.();
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
							commitOid: undefined,
							offset: -1
						});
						close();
					}}
					disabled={commitInsertion.current.isLoading}
				/>
				{#if stackLength && stackLength > 1}
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
							renameBranchModal?.show();
							close();
						}}
					/>
				{/if}
				{#if stackLength && stackLength > 1}
					<ContextMenuItem
						label="Delete"
						testId={TestId.BranchHeaderContextMenu_Delete}
						onclick={() => {
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
	{#if stackId}
		<BranchRenameModal
			{projectId}
			{stackId}
			bind:this={renameBranchModal}
			{branchName}
			isPushed={!!branch.remoteTrackingBranch}
		/>
		<DeleteBranchModal {projectId} {stackId} bind:this={deleteBranchModal} {branchName} />
	{/if}
{/if}
