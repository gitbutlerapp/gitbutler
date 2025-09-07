<script lang="ts">
	import { goto } from '$app/navigation';
	import BranchExplorer from '$components/BranchExplorer.svelte';
	import BranchView from '$components/BranchView.svelte';
	import BranchesViewBranch from '$components/BranchesViewBranch.svelte';
	import BranchesViewPr from '$components/BranchesViewPR.svelte';
	import BranchesViewStack from '$components/BranchesViewStack.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import PrBranchView from '$components/PRBranchView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import Scrollbar from '$components/Scrollbar.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import TargetCommitList from '$components/TargetCommitList.svelte';
	import UnappliedBranchView from '$components/UnappliedBranchView.svelte';
	import UnappliedCommitView from '$components/UnappliedCommitView.svelte';
	import BranchListCard from '$components/branchesPage/BranchListCard.svelte';
	import BranchesListGroup from '$components/branchesPage/BranchesListGroup.svelte';
	import CurrentOriginCard from '$components/branchesPage/CurrentOriginCard.svelte';
	import PRListCard from '$components/branchesPage/PRListCard.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { HorizontalPanner } from '$lib/dragging/horizontalPanner';
	import { isParsedError } from '$lib/error/parser';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import {
		createBranchSelection,
		createCommitSelection,
		type SelectionId
	} from '$lib/selection/key';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { AsyncButton, Button, Modal, TestId } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { BranchFilterOption, SidebarEntrySubject } from '$lib/branches/branchListing';
	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const forgeUserQuery = $derived(forge.current.user);
	const prService = $derived(forge.current.prService);
	const prUnit = $derived(prService?.unit);

	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);

	const baseBranchResult = $derived(baseBranchService.baseBranch(projectId));

	const selectedOption = persisted<BranchFilterOption>(
		'all',
		`branches-selectedOption-${projectId}`
	);

	let branchColumn = $state<HTMLDivElement>();
	let commitColumn = $state<HTMLDivElement>();
	let rightWrapper = $state<HTMLDivElement>();
	let branchViewLeftEl = $state<HTMLDivElement>();

	const selectionId: SelectionId | undefined = $derived.by(() => {
		const current = branchesState?.current;
		if (current.commitId) {
			return createCommitSelection({ commitId: current.commitId, stackId: current.stackId });
		}
		if (current.branchName) {
			return createBranchSelection({
				stackId: current.stackId,
				branchName: current.branchName,
				remote: current.remote
			});
		}
	});

	async function checkoutBranch() {
		const { branchName, remote, prNumber, hasLocal } = branchesState.current;
		const remoteRef = remote ? `refs/remotes/${remote}/${branchName}` : undefined;
		const branchRef = hasLocal ? `refs/heads/${branchName}` : remoteRef;
		if (branchRef) {
			await stackService.createVirtualBranchFromBranch({
				projectId,
				branch: branchRef,
				remote: remoteRef,
				prNumber
			});
			await baseBranchService.refreshBaseBranch(projectId);
		}
		goto(workspacePath(projectId));
	}

	async function deleteLocalBranch(branchName: string) {
		const hasLocal = branchesState.current.hasLocal;
		if (!hasLocal) {
			return;
		}

		await stackService.deleteLocalBranch({
			projectId,
			refname: `refs/heads/${branchName}`,
			givenName: branchName
		});

		// Unselect branch
		branchesState.set({});
		await baseBranchService.refreshBaseBranch(projectId);
	}

	let prBranch = $state<BranchesViewPr>();

	function applyFromFork() {
		prBranch?.applyPr();
	}

	let deleteLocalBranchModal = $state<Modal>();

	function handleDeleteLocalBranch(branchName: string) {
		deleteLocalBranchModal?.show(branchName);
	}

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			branchesState.set({});
			console.warn('Branches selection cleared');
		}
	}

	const horizontalPanner = $derived(rightWrapper ? new HorizontalPanner(rightWrapper) : undefined);

	$effect(() => {
		if (horizontalPanner) {
			const unsub = horizontalPanner.registerListeners();
			return () => unsub?.();
		}
	});

	let selectionPreviewScrollContainer: HTMLDivElement | undefined = $state();
</script>

<Modal
	testId={TestId.DeleteLocalBranchConfirmationModal}
	bind:this={deleteLocalBranchModal}
	title="Delete local branch"
	width="small"
	defaultItem={branchesState.current.branchName}
	onSubmit={async (close, branchName: string | undefined) => {
		if (branchName) {
			await deleteLocalBranch(branchName);
		}
		close();
	}}
>
	{#snippet children(branchName)}
		<p>Are you sure you want to delete the local changes inside the branch {branchName}?</p>
	{/snippet}

	{#snippet controls(close)}
		<Button
			testId={TestId.DeleteLocalBranchConfirmationModal_Cancel}
			kind="outline"
			type="reset"
			onclick={close}>Cancel</Button
		>
		<Button
			testId={TestId.DeleteLocalBranchConfirmationModal_Delete}
			style="error"
			type="submit"
			icon="bin">Delete</Button
		>
	{/snippet}
</Modal>

<ReduxResult {projectId} result={baseBranchResult.current}>
	{#snippet children(baseBranch)}
		{@const lastCommit = baseBranch.recentCommits.at(0)}
		{@const current = branchesState.current}
		{@const currentBranchName = current.branchName ?? baseBranch.shortName}
		{@const someBranchSelected = current.branchName !== undefined}
		{@const isTargetBranch =
			currentBranchName === baseBranch.shortName && current.prNumber === undefined}
		{@const inWorkspaceOrTargetBranch = current.inWorkspace || isTargetBranch}
		{@const isStackOrNormalBranchPreview =
			current.stackId || (current.branchName && !isTargetBranch)}
		{@const isNonLocalPr = !isStackOrNormalBranchPreview && current.prNumber !== undefined}

		<div class="branches-view">
			<div class="relative overflow-hidden radius-ml">
				<div
					bind:this={branchViewLeftEl}
					class="branches-view__left"
					use:focusable={{ list: true }}
				>
					<BranchesListGroup title="Current workspace target">
						<!-- TODO: We need an API for `commitsCount`! -->
						<CurrentOriginCard
							originName={baseBranch.branchName}
							lastCommit={lastCommit
								? {
										author: lastCommit.author,
										ago: getTimeAgo(lastCommit.createdAt, true),
										branch: baseBranch.shortName,
										sha: lastCommit.id.slice(0, 7)
									}
								: undefined}
							onclick={() => {
								branchesState.set({ branchName: baseBranch.shortName, isTarget: true });
							}}
							selected={(current.branchName === undefined ||
								current.branchName === baseBranch.shortName) &&
								current.prNumber === undefined}
						/>
					</BranchesListGroup>
					<BranchExplorer
						{projectId}
						bind:selectedOption={$selectedOption}
						forgeUser={forgeUserQuery.current.data}
					>
						{#snippet sidebarEntry(sidebarEntrySubject: SidebarEntrySubject)}
							{#if sidebarEntrySubject.type === 'branchListing'}
								<BranchListCard
									reviewUnit={prUnit}
									{projectId}
									branchListing={sidebarEntrySubject.subject}
									prs={sidebarEntrySubject.prs}
									selected={sidebarEntrySubject.subject.stack
										? current.branchName === sidebarEntrySubject.subject.stack.branches.at(0)
										: current.branchName === sidebarEntrySubject.subject.name}
									onclick={({ listing, pr }) => {
										if (listing.stack) {
											branchesState.set({
												stackId: listing.stack.id,
												branchName: listing.stack.branches.at(0),
												prNumber: pr?.number,
												inWorkspace: listing.stack.inWorkspace,
												hasLocal: listing.hasLocal
											});
										} else {
											branchesState.set({
												branchName: listing.name,
												prNumber: pr?.number,
												remote: listing.remotes.at(0),
												hasLocal: listing.hasLocal
											});
										}
									}}
								/>
							{:else}
								<PRListCard
									reviewUnit={prUnit}
									number={sidebarEntrySubject.subject.number}
									isDraft={sidebarEntrySubject.subject.draft}
									title={sidebarEntrySubject.subject.title}
									sourceBranch={sidebarEntrySubject.subject.sourceBranch}
									author={{
										name: sidebarEntrySubject.subject.author?.name,
										email: sidebarEntrySubject.subject.author?.email,
										gravatarUrl: sidebarEntrySubject.subject.author?.gravatarUrl
									}}
									modifiedAt={sidebarEntrySubject.subject.modifiedAt}
									selected={current.prNumber === sidebarEntrySubject.subject.number}
									onclick={(pr) => branchesState.set({ prNumber: pr.number })}
									noRemote
								/>
							{/if}
						{/snippet}
					</BranchExplorer>
				</div>
				<Resizer
					viewport={branchViewLeftEl}
					direction="right"
					minWidth={14}
					persistId="resizer-branchesWidth"
					defaultValue={24}
				/>
			</div>

			<div class="branches-view__right">
				<div class="right-wrapper hide-native-scrollbar dotted-pattern" bind:this={rightWrapper}>
					<div class="branch-column" bind:this={branchColumn} use:focusable={{ list: true }}>
						<!-- Apply branch -->
						{#if !inWorkspaceOrTargetBranch && someBranchSelected && !isNonLocalPr}
							{@const doesNotHaveLocalTooltip = current.hasLocal
								? undefined
								: 'No local branch to delete'}
							{@const doesNotHaveABranchNameTooltip = current.branchName
								? undefined
								: 'No branch selected to delete'}

							<div class="branches-actions">
								{#if !current.isTarget}
									<AsyncButton
										testId={TestId.BranchesViewApplyBranchButton}
										icon="workbench"
										shrinkable
										action={async () => {
											await checkoutBranch();
										}}
									>
										Apply to workspace
									</AsyncButton>
								{/if}

								<Button
									testId={TestId.BranchesViewDeleteLocalBranchButton}
									kind="outline"
									icon="bin-small"
									onclick={() => {
										if (current.branchName) {
											handleDeleteLocalBranch(current.branchName);
										}
									}}
									disabled={!current.hasLocal || !current.branchName}
									tooltip={doesNotHaveLocalTooltip ?? doesNotHaveABranchNameTooltip}
								>
									Delete local
								</Button>
							</div>
						{/if}

						{#if isNonLocalPr && !inWorkspaceOrTargetBranch}
							<div class="branches-actions">
								{#if !current.isTarget}
									<Button
										testId={TestId.BranchesViewApplyFromForkButton}
										icon="workbench"
										onclick={applyFromFork}
									>
										Apply PR to workspace
									</Button>
								{/if}
							</div>
						{/if}

						{#if isTargetBranch}
							<div class="commits" use:focusable={{ list: true }}>
								<TargetCommitList {projectId} />
							</div>
						{/if}

						{#if !isTargetBranch && someBranchSelected && !isNonLocalPr}
							<ConfigurableScrollableContainer>
								<div class="commits with-padding" use:focusable={{ list: true }}>
									{#if current.stackId}
										<BranchesViewStack {projectId} stackId={current.stackId} {onerror} />
									{:else if current.branchName}
										<BranchesViewBranch
											{projectId}
											branchName={current.branchName}
											remote={current.remote}
											{onerror}
										/>
									{/if}
								</div>
							</ConfigurableScrollableContainer>
						{/if}

						{#if isNonLocalPr && current.prNumber}
							<div class="commits" use:focusable={{ list: true }}>
								<BranchesViewPr
									bind:this={prBranch}
									{projectId}
									prNumber={current.prNumber}
									{onerror}
								/>
							</div>
						{/if}

						<Resizer
							viewport={branchColumn}
							persistId="branches-branch-column-1"
							direction="right"
							defaultValue={20}
							minWidth={10}
							maxWidth={30}
						/>
					</div>

					{#if current.commitId || (current.branchName && ((current.inWorkspace && current.stackId) || !current.isTarget)) || current.prNumber}
						<div class="commit-column" bind:this={commitColumn} class:non-local-pr={isNonLocalPr}>
							{#if current.commitId}
								<UnappliedCommitView {projectId} commitId={current.commitId} />
								<Resizer
									viewport={commitColumn}
									persistId="branches-branch-column-2"
									direction="right"
									defaultValue={20}
									minWidth={10}
									maxWidth={30}
								/>
							{:else if current.branchName}
								{#if current.inWorkspace && current.stackId}
									<BranchView
										{projectId}
										laneId="branches-view"
										branchName={current.branchName}
										stackId={current.stackId}
										active
										{onerror}
									/>
								{:else if !current.isTarget}
									<UnappliedBranchView
										{projectId}
										branchName={current.branchName}
										stackId={current.stackId}
										remote={current.remote}
										prNumber={current.prNumber}
										{onerror}
									/>
								{/if}
								<Resizer
									viewport={commitColumn}
									persistId="branches-branch-column-2"
									direction="right"
									defaultValue={20}
									minWidth={10}
									maxWidth={30}
								/>
							{:else if current.prNumber}
								<PrBranchView {projectId} prNumber={current.prNumber} {onerror} />
							{/if}
						</div>
					{/if}

					{#if !isNonLocalPr}
						<div class="preview-selection" use:focusable>
							<ConfigurableScrollableContainer
								bind:viewport={selectionPreviewScrollContainer}
								zIndex="var(--z-lifted)"
							>
								<SelectionView
									scrollContainer={selectionPreviewScrollContainer}
									testId={TestId.BranchesSelectionView}
									{projectId}
									{selectionId}
									bottomBorder
								/>
							</ConfigurableScrollableContainer>
						</div>
					{/if}
				</div>

				<Scrollbar viewport={rightWrapper} horz />
			</div>
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.branches-view {
		display: flex;
		position: relative;
		width: 100%;
		height: 100%;
		gap: 8px;
	}

	.branches-view__left,
	.branches-view__right {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		height: 100%;
		max-height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.right-wrapper {
		display: flex;
		position: relative;
		height: 100%;
		margin-right: -1px;
		margin-left: -1px;
		overflow: hidden;
		overflow-x: auto;
	}

	.branch-column {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		max-height: 100%;
		border-right: 1px solid var(--clr-border-2);
		border-left: 1px solid var(--clr-border-2);
	}

	.commit-column {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		max-height: 100%;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-2);

		&.non-local-pr {
			flex-grow: 1;
			border-right: none;
		}
	}

	.commits {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;

		&.with-padding {
			padding: 12px;
		}
	}

	.branches-actions {
		display: flex;
		padding: 12px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.preview-selection {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		min-width: 460px;
		min-height: 100%;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-2);
	}
</style>
