<script lang="ts">
	import { goto } from "$app/navigation";
	import BranchExplorer from "$components/branchesPage/BranchExplorer.svelte";
	import BranchListCard from "$components/branchesPage/BranchListCard.svelte";
	import BranchesListGroup from "$components/branchesPage/BranchesListGroup.svelte";
	import BranchesViewPr from "$components/branchesPage/BranchesViewPR.svelte";
	import BranchesViewStack from "$components/branchesPage/BranchesViewStack.svelte";
	import CurrentOriginCard from "$components/branchesPage/CurrentOriginCard.svelte";
	import PRListCard from "$components/branchesPage/PRListCard.svelte";
	import UnappliedCommitView from "$components/commit/UnappliedCommitView.svelte";
	import MultiDiffView from "$components/diff/MultiDiffView.svelte";
	import PrBranchView from "$components/forge/PrDetailsDrawer.svelte";
	import AppScrollableContainer from "$components/shared/AppScrollableContainer.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import Resizer from "$components/shared/Resizer.svelte";
	import SashLayer from "$components/shared/SashLayer.svelte";
	import BranchesViewBranch from "$components/views/BranchesViewBranch.svelte";
	import TargetCommitList from "$components/views/TargetCommitList.svelte";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
	import { newBranchApplyFeature } from "$lib/config/uiFeatureFlags";
	import { isNormalizedError } from "$lib/error/normalizedError";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { useGitHubForgeUser } from "$lib/forge/github/hooks.svelte";
	import { useGitLabForgeUser } from "$lib/forge/gitlab/hooks.svelte";
	import { workspacePath } from "$lib/routes/routes.svelte";
	import { handleApplyOutcome, handleCreateBranchFromBranchOutcome } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { combineResults } from "$lib/state/helpers";
	import { inject } from "@gitbutler/core/context";
	import { persisted } from "@gitbutler/shared/persisted";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import { AsyncButton, Button, Modal, TestId } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { getTimeAgo } from "@gitbutler/ui/utils/timeAgo";
	import { untrack } from "svelte";
	import type { BranchFilterOption, SidebarEntrySubject } from "$lib/branches/branchListing";
	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	type BranchesSelection =
		| {
				type: "branch";
				branchName: string;
				remote?: string;
				stackId?: string;
				commitId?: string;
		  }
		| { type: "pr"; prNumber: number }
		| { type: "target"; commitId?: string };

	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const reviewUnitAbbr = $derived(forgeInfo?.unit.abbr ?? "PR");
	// Call both hooks at init (they inject()/getContext(), which must not
	// run inside a $derived re-computation); select reactively by forge.
	const projectIdRef = reactive(() => projectId);
	const githubUser = useGitHubForgeUser(projectIdRef);
	const gitlabUser = useGitLabForgeUser(projectIdRef);
	const forgeUser = $derived.by(() => {
		switch (forgeInfo?.name) {
			case "github":
				return githubUser.user.current;
			case "gitlab":
				return gitlabUser.user.current;
			default:
				return undefined;
		}
	});
	const prUnit = $derived(forgeInfo?.unit);
	const branchService = inject(BRANCH_SERVICE);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const selectedOption = persisted<BranchFilterOption>(
		"all",
		`branches-selectedOption-${untrack(() => projectId)}`,
	);

	let selection = $state<BranchesSelection>({ type: "target" });

	let branchColumn = $state<HTMLDivElement>();
	let branchViewLeftEl = $state<HTMLDivElement>();

	const LEFT_PANEL_RESIZER = {
		minWidth: 16,
		maxWidth: 40,
		defaultValue: 24,
	};

	const BRANCH_COLUMN_RESIZER = {
		minWidth: 20,
		maxWidth: 30,
		defaultValue: 20,
	};

	async function applyBranchToWorkspace(args: {
		branchName: string;
		remote?: string;
		prNumber?: number;
		hasLocal: boolean;
	}) {
		const { remote, hasLocal, branchName, prNumber } = args;
		const remoteRef = remote ? `refs/remotes/${remote}/${branchName}` : undefined;
		const branchRef = hasLocal ? `refs/heads/${branchName}` : remoteRef;
		if (branchRef) {
			if ($newBranchApplyFeature) {
				const outcome = await stackService.branchApply({
					projectId,
					existingBranch: branchRef,
				});
				handleApplyOutcome(outcome);
			} else {
				const outcome = await stackService.createVirtualBranchFromBranch({
					projectId,
					branch: branchRef,
					prNumber,
				});
				handleCreateBranchFromBranchOutcome(outcome);
			}
			await baseBranchService.refreshBaseBranch(projectId);
		}
		goto(workspacePath(projectId));
	}

	async function deleteLocalBranch(branchName: string) {
		await stackService.deleteLocalBranch({
			projectId,
			refname: `refs/heads/${branchName}`,
			givenName: branchName,
		});
		// Unselect branch
		await baseBranchService.refreshBaseBranch(projectId);
	}

	let prBranch = $state<BranchesViewPr>();

	async function applyFromFork() {
		await prBranch?.applyPr();
	}

	let deleteLocalBranchModal = $state<Modal>();

	function handleDeleteLocalBranch(branchName: string) {
		deleteLocalBranchModal?.show(branchName);
	}

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isNormalizedError(err) && err.code === "BranchNotFound") {
			selection = { type: "target" };
			console.warn("Branches selection cleared");
		}
	}

	let multiDiffView = $state<MultiDiffView>();
</script>

{#snippet branchActions(
	branchName: string,
	remote: string | undefined,
	hasLocal: boolean,
	prNumber: number | undefined,
)}
	<div class="branch-actions">
		<AsyncButton
			testId={TestId.BranchesViewApplyBranchButton}
			icon="workbench"
			shrinkable
			action={async () => {
				await applyBranchToWorkspace({
					remote,
					branchName,
					hasLocal,
					prNumber,
				});
			}}
		>
			Apply to workspace
		</AsyncButton>
		<Button
			testId={TestId.BranchesViewDeleteLocalBranchButton}
			kind="outline"
			icon="bin"
			onclick={() => {
				handleDeleteLocalBranch(branchName);
			}}
			disabled={!hasLocal}
			tooltip={hasLocal ? undefined : "No local branch to delete"}
		>
			Delete local
		</Button>
	</div>
{/snippet}

<Modal
	testId={TestId.DeleteLocalBranchConfirmationModal}
	bind:this={deleteLocalBranchModal}
	title="Delete local branch"
	width="small"
	defaultItem={selection.type === "branch" ? selection.branchName : undefined}
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

<SashLayer>
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
											ago: getTimeAgo(new Date(lastCommit.committedAt), true),
											branch: baseBranch.shortName,
											sha: lastCommit.id.slice(0, 7),
										}
									: undefined}
								onclick={() => {
									selection = { type: "target" };
								}}
								selected={selection.type === "target"}
							/>
						</BranchesListGroup>
						<BranchExplorer
							{projectId}
							bind:selectedOption={$selectedOption}
							{forgeUser}
							{baseBranch}
						>
							{#snippet sidebarEntry(sidebarEntrySubject: SidebarEntrySubject)}
								{#if sidebarEntrySubject.type === "branchListing"}
									<BranchListCard
										reviewUnit={prUnit}
										{projectId}
										branchListing={sidebarEntrySubject.subject}
										prs={sidebarEntrySubject.prs}
										selected={selection.type === "branch"
											? selection.branchName === sidebarEntrySubject.subject.name
											: false}
										onclick={({ listing }) => {
											if (listing.stack) {
												selection = {
													type: "branch",
													branchName: listing.name,
													stackId: listing.stack.id,
												};
											} else {
												selection = {
													type: "branch",
													branchName: listing.name,
													remote: listing.remotes.at(0),
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
											gravatarUrl: sidebarEntrySubject.subject.author?.gravatarUrl,
										}}
										modifiedAt={sidebarEntrySubject.subject.modifiedAt}
										mergedAt={sidebarEntrySubject.subject.mergedAt}
										closedAt={sidebarEntrySubject.subject.closedAt}
										selected={selection.type === "pr" &&
											selection.prNumber === sidebarEntrySubject.subject.number}
										onclick={(pr) => (selection = { type: "pr", prNumber: pr.number })}
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
				minWidth={LEFT_PANEL_RESIZER.minWidth}
				maxWidth={LEFT_PANEL_RESIZER.maxWidth}
				persistId="resizer-branchesWidth"
				defaultValue={LEFT_PANEL_RESIZER.defaultValue}
			/>
		</div>

		<div class="branches-view__right">
			<div class="right-wrapper dotted-pattern">
				{#if selection.type === "target"}
					<div class="branch-column" bind:this={branchColumn} use:focusable={{ vertical: true }}>
						<TargetCommitList
							{projectId}
							onclick={(commitId) => (selection = { type: "target", commitId })}
							onFileClick={(index) => {
								multiDiffView?.jumpToIndex(index);
							}}
						/>
						<Resizer
							viewport={branchColumn}
							persistId="branches-branch-column-1"
							direction="right"
							defaultValue={BRANCH_COLUMN_RESIZER.defaultValue}
							minWidth={BRANCH_COLUMN_RESIZER.minWidth}
							maxWidth={BRANCH_COLUMN_RESIZER.maxWidth}
						/>
					</div>
				{:else}
					<AppScrollableContainer>
						<div class="branch-column" bind:this={branchColumn} use:focusable={{ vertical: true }}>
							{#if selection.type === "branch"}
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

										{#if stackId}
											{@const selectedStack = stackService.stackById(projectId, stackId)}
											<ReduxResult result={selectedStack.result} {projectId} {stackId} {onerror}>
												{#snippet children(liveStack)}
													{@const stackIsLive = liveStack !== null}
													{@const isAppliedInCurrentWorkspace = inWorkspace === true && stackIsLive}
													{#if branchName && !isAppliedInCurrentWorkspace}
														{@render branchActions(branchName, remote, hasLocal, prNumber)}
													{/if}

													{#if stackIsLive}
														<BranchesViewStack
															{projectId}
															{stackId}
															isTarget={false}
															inWorkspace={inWorkspace ?? false}
															selectedCommitId={selection.type === "branch"
																? selection.commitId
																: undefined}
															onCommitClick={(commitId) => {
																selection = {
																	type: "branch",
																	branchName,
																	remote,
																	stackId,
																	commitId,
																};
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
															inWorkspace={false}
															selectedCommitId={selection.type === "branch"
																? selection.commitId
																: undefined}
															onCommitClick={(commitId) => {
																selection = {
																	type: "branch",
																	branchName,
																	remote,
																	stackId,
																	commitId,
																};
															}}
															onFileClick={(index) => {
																multiDiffView?.jumpToIndex(index);
															}}
															{onerror}
														/>
													{/if}
												{/snippet}
											</ReduxResult>
										{:else if branchName}
											{#if inWorkspace !== true}
												{@render branchActions(branchName, remote, hasLocal, prNumber)}
											{/if}
											<BranchesViewBranch
												{projectId}
												{branchName}
												{remote}
												inWorkspace={inWorkspace ?? false}
												selectedCommitId={selection.type === "branch"
													? selection.commitId
													: undefined}
												onCommitClick={(commitId) => {
													selection = { type: "branch", branchName, remote, stackId, commitId };
												}}
												onFileClick={(index) => {
													multiDiffView?.jumpToIndex(index);
												}}
												{onerror}
											/>
										{/if}
									{/snippet}
								</ReduxResult>
							{:else if selection.type === "pr"}
								{@const prNumber = selection.prNumber}
								<div class="branch-actions">
									<AsyncButton
										testId={TestId.BranchesViewApplyFromForkButton}
										icon="workbench"
										action={applyFromFork}
									>
										Apply {reviewUnitAbbr} to workspace
									</AsyncButton>
								</div>
								<BranchesViewPr bind:this={prBranch} {projectId} {prNumber} {onerror} />
							{/if}
							<Resizer
								viewport={branchColumn}
								persistId="branches-branch-column-1"
								direction="right"
								defaultValue={BRANCH_COLUMN_RESIZER.defaultValue}
								minWidth={BRANCH_COLUMN_RESIZER.minWidth}
								maxWidth={BRANCH_COLUMN_RESIZER.maxWidth}
							/>
						</div>
					</AppScrollableContainer>
				{/if}

				<div class="commit-column">
					{#if selection.type === "branch" && selection.commitId}
						{@const { commitId } = selection}
						{@const changesQuery = stackService.commitChanges(projectId, commitId)}
						<UnappliedCommitView {projectId} {commitId} />
						<ReduxResult {projectId} result={changesQuery.result}>
							{#snippet children(result, { projectId })}
								<MultiDiffView
									bind:this={multiDiffView}
									selectionId={{
										type: "commit",
										commitId: commitId,
									}}
									changes={result.changes}
									{projectId}
									draggable={true}
									selectable={false}
								/>
							{/snippet}
						</ReduxResult>
					{:else if selection.type === "pr"}
						{@const prNumber = selection.prNumber}
						<PrBranchView {projectId} {prNumber} {onerror} />
					{:else if selection.type === "target"}
						{@const commitId = selection.commitId}
						{#if commitId}
							{@const changesQuery = stackService.commitChanges(projectId, commitId)}
							<UnappliedCommitView {projectId} {commitId} />
							<ReduxResult {projectId} result={changesQuery.result}>
								{#snippet children(result, { projectId })}
									<MultiDiffView
										bind:this={multiDiffView}
										selectionId={{
											type: "commit",
											commitId: commitId,
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
</SashLayer>

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
		border: 1px solid var(--border-2);
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
		margin-bottom: 12px;
		padding: 12px;
		gap: 6px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background-color: var(--bg-1);
	}
</style>
