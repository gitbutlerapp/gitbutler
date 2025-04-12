<script lang="ts">
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import { AIService } from '$lib/ai/service';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { type CommitStatus } from '$lib/commits/commit';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		branchName: string;
		seriesCount: number;
		isTopBranch: boolean;
		isPushed: boolean;
		pr?: DetailedPullRequest;
		branchType: CommitStatus;
		descriptionOption?: boolean;
		descriptionString?: string;
		stackId: string;
		toggleDescription?: () => Promise<void>;
		onGenerateBranchName: () => void;
		onAddDependentSeries?: () => void;
		onOpenInBrowser?: () => void;
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
		onOpenInBrowser
	}: Props = $props();

	const [aiService, stackService] = inject(AIService, StackService);
	const allCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const isConflicted = $derived(
		allCommits.current.data?.some((commit) => commit.hasConflicts) ?? false
	);

	const [removeBranch, branchRemovalOp] = stackService.removeBranch;

	let deleteSeriesModal: Modal;
	let renameBranchModal: BranchRenameModal;
	let aiConfigurationValid = $state(false);

	$effect(() => {
		setAIConfigurationValid();
	});

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	export function showSeriesRenameModal() {
		renameBranchModal.show();
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
				renameBranchModal.show();
				contextMenuEl?.close();
			}}
		/>
	{/if}
	{#if seriesCount > 1}
		<ContextMenuItem
			label="Delete"
			testId={TestId.BranchHeaderContextMenu_Delete}
			onclick={() => {
				deleteSeriesModal.show(stackId);
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

<BranchRenameModal {projectId} {stackId} {branchName} bind:this={renameBranchModal} {isPushed} />

<Modal
	width="small"
	title="Delete branch"
	bind:this={deleteSeriesModal}
	onSubmit={async (close) => {
		await removeBranch({
			projectId,
			stackId,
			branchName
		});
		close();
	}}
>
	{#snippet children()}
		Are you sure you want to delete <code class="code-string">{branchName}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={branchRemovalOp.current.isLoading}>Delete</Button>
	{/snippet}
</Modal>
