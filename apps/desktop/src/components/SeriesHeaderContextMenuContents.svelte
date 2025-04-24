<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		branchName: string;
		seriesCount: number;
		isTopBranch: boolean;
		isPushed: boolean;
		pr?: DetailedPullRequest;
		branchType: CommitStatusType;
		descriptionOption?: boolean;
		descriptionString?: string;
		stackId: string;
		toggleDescription?: () => Promise<void>;
		onGenerateBranchName: () => void;
		onAddDependentSeries?: () => void;
		onOpenInBrowser?: () => void;

		showBranchRenameModal: () => void;
		showDeleteBranchModal: () => void;
	}

	let {
		projectId,
		contextMenuEl = $bindable(),
		isTopBranch,
		seriesCount,
		isPushed,
		branchName,
		pr,
		branchType,
		descriptionOption = true,
		descriptionString,
		stackId,
		toggleDescription,
		onGenerateBranchName,
		onAddDependentSeries,
		onOpenInBrowser,
		showBranchRenameModal,
		showDeleteBranchModal
	}: Props = $props();

	const [aiService, stackService] = inject(AIService, StackService);
	const allCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const isConflicted = $derived(
		allCommits.current.data?.some((commit) => commit.hasConflicts) ?? false
	);

	let aiConfigurationValid = $state(false);

	$effect(() => {
		setAIConfigurationValid();
	});

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}
</script>

{#if isTopBranch}
	<ContextMenuSection>
		<ContextMenuItem
			label="Add dependent branch"
			testId={TestId.BranchHeaderContextMenu_AddDependentBranch}
			onclick={() => {
				onAddDependentSeries?.();
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
{/if}
<ContextMenuSection>
	{#if isPushed}
		<ContextMenuItem
			label="Open in browser"
			testId={TestId.BranchHeaderContextMenu_OpenInBrowser}
			onclick={() => {
				onOpenInBrowser?.();
				contextMenuEl?.close();
			}}
		/>
	{/if}
	<ContextMenuItem
		label="Copy branch name"
		testId={TestId.BranchHeaderContextMenu_CopyBranchName}
		onclick={() => {
			writeClipboard(branchName);
			contextMenuEl?.close();
		}}
	/>
</ContextMenuSection>
<ContextMenuSection>
	<ContextMenuItem
		label="Squash all commits"
		testId={TestId.BranchHeaderContextMenu_SquashAllCommits}
		onclick={async () => {
			await stackService.squashAllCommits({
				projectId,
				stackId,
				branchName
			});
			contextMenuEl?.close();
		}}
		disabled={isConflicted}
		tooltip={isConflicted ? 'This branch has conflicts' : undefined}
	/>
	{#if descriptionOption}
		<ContextMenuItem
			label={`${!descriptionString ? 'Add' : 'Remove'} description`}
			testId={TestId.BranchHeaderContextMenu_AddRemoveDescription}
			onclick={async () => {
				await toggleDescription?.();
				contextMenuEl?.close();
			}}
		/>
	{/if}
	{#if $aiGenEnabled && aiConfigurationValid && !isPushed}
		<ContextMenuItem
			label="Generate branch name"
			testId={TestId.BranchHeaderContextMenu_GenerateBranchName}
			onclick={() => {
				onGenerateBranchName();
				contextMenuEl?.close();
			}}
		/>
	{/if}
	{#if branchType !== 'Integrated'}
		<ContextMenuItem
			label="Rename"
			testId={TestId.BranchHeaderContextMenu_Rename}
			onclick={async () => {
				showBranchRenameModal();
				contextMenuEl?.close();
			}}
		/>
	{/if}
	{#if seriesCount > 1}
		<ContextMenuItem
			label="Delete"
			testId={TestId.BranchHeaderContextMenu_Delete}
			onclick={() => {
				showDeleteBranchModal();
				contextMenuEl?.close();
			}}
		/>
	{/if}
</ContextMenuSection>
{#if pr?.htmlUrl}
	<ContextMenuSection>
		<ContextMenuItem
			label="Open PR in browser"
			testId={TestId.BranchHeaderContextMenu_OpenPRInBrowser}
			onclick={() => {
				openExternalUrl(pr.htmlUrl);
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Copy PR link"
			testId={TestId.BranchHeaderContextMenu_CopyPRLink}
			onclick={() => {
				writeClipboard(pr.htmlUrl);
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
{/if}
