<script lang="ts">
	import { goto } from '$app/navigation';
	import BranchExplorer from '$components/BranchExplorer.svelte';
	import BranchesViewBranch from '$components/BranchesViewBranch.svelte';
	import BranchesViewPr from '$components/BranchesViewPR.svelte';
	import BranchesViewStack from '$components/BranchesViewStack.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import MultiDiffView from '$components/MultiDiffView.svelte';
	import PrBranchView from '$components/PRBranchView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import TargetCommitList from '$components/TargetCommitList.svelte';
	import UnappliedCommitView from '$components/UnappliedCommitView.svelte';
	import BranchListCard from '$components/branchesPage/BranchListCard.svelte';
	import BranchesListGroup from '$components/branchesPage/BranchesListGroup.svelte';
	import CurrentOriginCard from '$components/branchesPage/CurrentOriginCard.svelte';
	import PRListCard from '$components/branchesPage/PRListCard.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { isParsedError } from '$lib/error/parser';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { handleCreateBranchFromBranchOutcome } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
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

	type BranchesSelection =
		| {
				type: 'branch';
				branchName: string;
				remote?: string;
				stackId?: string;
				commitId?: string;
		  }
		| { type: 'pr'; prNumber: number }
		| { type: 'target'; commitId?: string };

	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const forgeUserQuery = $derived(forge.current.user);
	const prService = $derived(forge.current.prService);
	const prUnit = $derived(prService?.unit);
	const branchService = inject(BRANCH_SERVICE);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const selectedOption = persisted<BranchFilterOption>(
		'all',
		`branches-selectedOption-${projectId}`
	);

	let selection = $state<BranchesSelection>({ type: 'target' });

	let branchColumn = $state<HTMLDivElement>();
	let branchViewLeftEl = $state<HTMLDivElement>();

	async function checkoutBranch(args: {
		branchName: string;
		remote?: string;
		prNumber?: number;
		hasLocal: boolean;
	}) {
		const { remote, hasLocal, branchName, prNumber } = args;
		const remoteRef = remote ? `refs/remotes/${remote}/${branchName}` : undefined;
		const branchRef = hasLocal ? `refs/heads/${branchName}` : remoteRef;
		if (branchRef) {
			const outcome = await stackService.createVirtualBranchFromBranch({
				projectId,
				branch: branchRef,
				remote: remoteRef,
				prNumber
			});
			handleCreateBranchFromBranchOutcome(outcome);
			await baseBranchService.refreshBaseBranch(projectId);
		}
		goto(workspacePath(projectId));
	}

	async function deleteLocalBranch(branchName: string) {
		await stackService.deleteLocalBranch({
			projectId,
			refname: `refs/heads/${branchName}`,
			givenName: branchName
		});
		// Unselect branch
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
			selection = { type: 'target' };
			console.warn('Branches selection cleared');
		}
	}

	let multiDiffView = $state<MultiDiffView>();
</script>

<Modal
	testId={TestId.DeleteLocalBranchConfirmationModal}
	bind:this={deleteLocalBranchModal}
	title="Delete local branch"
	width="small"
	defaultItem={selection.type === 'branch' ? selection.branchName : undefined}
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
			style="danger"
			type="submit"
			icon="bin">Delete</Button
		>
	{/snippet}
</Modal>

<div class="branches-view" data-testid={TestId.BranchesView}>
	<div class="relative overflow-hidden radius-ml">
		<div
			bind:this={branchViewLeftEl}
			class="branches-view__left"
			use:focusable={{ vertical: true }}
		>
			<ReduxResult {projectId} result={baseBranchQuery.result}>
				{#snippet children(baseBranch)}
					{@const lastCommit = baseBranch.recentCommits.at(0)}
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
								selection = { type: 'target' };
							}}
							selected={selection.type === 'target'}
						/>
					</BranchesListGroup>
					<BranchExplorer
						{projectId}
						bind:selectedOption={$selectedOption}
						forgeUser={forgeUserQuery.response}
						{baseBranch}
					>
						{#snippet sidebarEntry(sidebarEntrySubject: SidebarEntrySubject)}
							{#if sidebarEntrySubject.type === 'branchListing'}
								<BranchListCard
									reviewUnit={prUnit}
									{projectId}
									branchListing={sidebarEntrySubject.subject}
									prs={sidebarEntrySubject.prs}
									selected={selection.type === 'branch'
										? sidebarEntrySubject.subject.stack
											? selection.branchName === sidebarEntrySubject.subject.stack.branches.at(0)
											: selection.branchName === sidebarEntrySubject.subject.name
										: false}
									onclick={({ listing }) => {
										if (listing.stack) {
											selection = {
												type: 'branch',
												branchName: listing.stack.branches[0]!,
												stackId: listing.stack.id
											};
										} else {
											selection = {
												type: 'branch',
												branchName: listing.name,
												remote: listing.remotes.at(0)
											};
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
									selected={selection.type === 'pr' &&
										selection.prNumber === sidebarEntrySubject.subject.number}
									onclick={(pr) => (selection = { type: 'pr', prNumber: pr.number })}
									noRemote
								/>
							{/if}
						{/snippet}
					</BranchExplorer>
				{/snippet}
			</ReduxResult>
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
		<div class="right-wrapper dotted-pattern">
			{#if selection.type === 'target'}
				<div class="branch-column" bind:this={branchColumn} use:focusable={{ vertical: true }}>
					<TargetCommitList
						{projectId}
						onclick={(commitId) => (selection = { type: 'target', commitId })}
						onFileClick={(index) => {
							multiDiffView?.jumpToIndex(index);
						}}
					/>
					<Resizer
						viewport={branchColumn}
						persistId="branches-branch-column-1"
						direction="right"
						defaultValue={20}
						minWidth={10}
						maxWidth={30}
					/>
				</div>
			{:else}
				<ConfigurableScrollableContainer>
					<div class="branch-column" bind:this={branchColumn} use:focusable={{ vertical: true }}>
						{#if selection.type === 'branch'}
							{@const { stackId, branchName, remote } = selection}
							{@const selectedBranch = branchService.get(projectId, selection.branchName)}
							{@const listing = branchService.listingByName(projectId, selection.branchName)}

							<ReduxResult
								{projectId}
								result={combineResults(selectedBranch.result, listing.result)}
							>
								{#snippet children([branch, listing])}
									{@const prNumber = branch.stack?.pullRequests[branchName]}
									{@const inWorkspace = branch.stack?.inWorkspace}
									{@const hasLocal = listing.hasLocal}
									<!-- Apply branch -->

									{#if branchName && !inWorkspace}
										<div class="branch-actions">
											<AsyncButton
												testId={TestId.BranchesViewApplyBranchButton}
												icon="workbench"
												shrinkable
												action={async () => {
													await checkoutBranch({ remote, branchName, hasLocal, prNumber });
												}}
											>
												Apply to workspace
											</AsyncButton>
											<Button
												testId={TestId.BranchesViewDeleteLocalBranchButton}
												kind="outline"
												icon="bin-small"
												onclick={() => {
													if (branchName) {
														handleDeleteLocalBranch(branchName);
													}
												}}
												disabled={!hasLocal || !branchName}
												tooltip={listing.hasLocal ? undefined : 'No local branch to delete'}
											>
												Delete local
											</Button>
										</div>
									{/if}

									{#if stackId}
										<BranchesViewStack
											{projectId}
											{stackId}
											isTarget={false}
											inWorkspace={inWorkspace ?? false}
											selectedCommitId={selection.type === 'branch'
												? selection.commitId
												: undefined}
											onCommitClick={(commitId) => {
												selection = { type: 'branch', branchName, remote, stackId, commitId };
											}}
											onFileClick={(index) => {
												multiDiffView?.jumpToIndex(index);
											}}
											{onerror}
										/>
									{:else if branchName}
										<BranchesViewBranch
											{projectId}
											{branchName}
											{remote}
											inWorkspace={inWorkspace ?? false}
											selectedCommitId={selection.type === 'branch'
												? selection.commitId
												: undefined}
											onCommitClick={(commitId) => {
												selection = { type: 'branch', branchName, remote, stackId, commitId };
											}}
											onFileClick={(index) => {
												multiDiffView?.jumpToIndex(index);
											}}
											{onerror}
										/>
									{/if}
								{/snippet}
							</ReduxResult>
						{:else if selection.type === 'pr'}
							{@const prNumber = selection.prNumber}
							<div class="branch-actions">
								<Button
									testId={TestId.BranchesViewApplyFromForkButton}
									icon="workbench"
									onclick={applyFromFork}
								>
									Apply {forge.reviewUnitAbbr} to workspace
								</Button>
							</div>
							<BranchesViewPr bind:this={prBranch} {projectId} {prNumber} {onerror} />
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
				</ConfigurableScrollableContainer>
			{/if}

			<div class="commit-column">
				{#if selection.type === 'branch' && selection.commitId}
					{@const { commitId } = selection}
					{@const changesQuery = stackService.commitChanges(projectId, commitId)}
					<UnappliedCommitView {projectId} {commitId} />
					<ReduxResult {projectId} result={changesQuery.result}>
						{#snippet children(result, { projectId })}
							<MultiDiffView
								bind:this={multiDiffView}
								selectionId={{
									type: 'commit',
									commitId: commitId
								}}
								changes={result.changes}
								{projectId}
								draggable={true}
								selectable={false}
							/>
						{/snippet}
					</ReduxResult>
				{:else if selection.type === 'pr'}
					{@const prNumber = selection.prNumber}
					<PrBranchView {projectId} {prNumber} {onerror} />
				{:else if selection.type === 'target'}
					{@const commitId = selection.commitId}
					{#if commitId}
						{@const changesQuery = stackService.commitChanges(projectId, commitId)}
						<UnappliedCommitView {projectId} {commitId} />
						<ReduxResult {projectId} result={changesQuery.result}>
							{#snippet children(result, { projectId })}
								<MultiDiffView
									bind:this={multiDiffView}
									selectionId={{
										type: 'commit',
										commitId: commitId
									}}
									changes={result.changes}
									{projectId}
									draggable={true}
									selectable={false}
								/>
							{/snippet}
						</ReduxResult>
					{/if}
				{/if}
			</div>
		</div>
	</div>
</div>

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
		overflow: hidden;
	}

	.branch-column {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		max-height: 100%;
		padding: 12px;
		gap: 12px;
	}

	.commit-column {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		max-height: 100%;
		padding: 12px;
		padding-left: 0;
		overflow: hidden;
		gap: 12px;
	}

	.branch-actions {
		display: flex;
		padding: 12px;
		gap: 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}
</style>
