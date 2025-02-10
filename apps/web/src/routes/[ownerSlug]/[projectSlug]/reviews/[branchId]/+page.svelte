<script lang="ts">
	import ChangeIndexCard from '$lib/components/changes/ChangeIndexCard.svelte';
	import Factoid from '$lib/components/infoFlexRow/Factoid.svelte';
	import InfoFlexRow from '$lib/components/infoFlexRow/InfoFlexRow.svelte';
	import BranchStatusBadge from '$lib/components/review/BranchStatusBadge.svelte';
	import CommitsGraph from '$lib/components/review/CommitsGraph.svelte';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import {
		BranchStatus,
		getContributorsWithAvatars,
		type Branch
	} from '@gitbutler/shared/branches/types';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, and, map } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import toasts from '@gitbutler/ui/toasts';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { goto } from '$app/navigation';

	dayjs.extend(relativeTime);

	interface Props {
		data: ProjectReviewParameters;
	}

	let { data }: Props = $props();

	const latestBranchLookupService = getContext(LatestBranchLookupService);
	const branchService = getContext(BranchService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);
	const userService = getContext(UserService);
	const user = $derived(userService.user);

	const branchUuid = $derived(
		lookupLatestBranchUuid(
			appState,
			latestBranchLookupService,
			data.ownerSlug,
			data.projectSlug,
			data.branchId
		)
	);

	const branch = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getBranchReview(appState, branchService, branchUuid);
		})
	);

	const isBranchAuthor = $derived(
		map(branch?.current, (branch) => {
			return branch.contributors.some(
				(contributor) => contributor.user?.id !== undefined && contributor.user?.id === $user?.id
			);
		})
	);

	const contributors = $derived(
		isFound(branch?.current)
			? getContributorsWithAvatars(branch.current.value)
			: Promise.resolve([])
	);

	function visitFirstCommit(branch: Branch) {
		if ((branch.patchIds?.length || 0) === 0) return;

		goto(routes.projectReviewBranchCommitPath({ ...data, changeId: branch.patchIds.at(-1)! }));
	}

	let editingSummary = $state(false);
	let summary = $state('');
	let title = $state('');

	function editSummary() {
		if (!isFound(branch?.current)) return;
		// Make sure we're not dealing with a reference to the origional
		summary = structuredClone(branch.current.value.description || '');
		title = structuredClone(branch.current.value.title || '');
		editingSummary = true;
	}

	function abortEditingSummary() {
		if (!confirm('Canceling will lose any changes made')) {
			return;
		}

		editingSummary = false;
	}

	async function saveSummary() {
		if (!isFound(branch?.current)) return;

		try {
			await branchService.updateBranch(branch.current.value.uuid, {
				title: title,
				description: summary
			});
			toasts.success('Updated review status');
		} finally {
			editingSummary = false;
		}
	}

	async function updateStatus(status: BranchStatus.Active | BranchStatus.Closed) {
		if (!isFound(branch?.current)) return;

		await branchService.updateBranch(branch.current.value.uuid, {
			status
		});
		toasts.success('Saved review summary');
	}

	function copyLocation() {
		copyToClipboard(location.href);
	}
</script>

{#snippet startReview(branch: Branch)}
	{#if (branch.stackSize || 0) > 0 && isBranchAuthor === false}
		<Button style="pop" icon="play" onclick={() => visitFirstCommit(branch)}>Start review</Button>
	{/if}
{/snippet}

<svelte:head>
	<title>Review: {data.ownerSlug}/{data.projectSlug}</title>
	<meta property="og:title" content="GitButler Review: {data.ownerSlug}/{data.projectSlug}" />
	<meta property="og:description" content="GitButler code review" />
</svelte:head>

<Loading loadable={and([branchUuid?.current, branch?.current])}>
	{#snippet children(branch)}
		{console.log(branch)}
		<div class="layout">
			<div class="information">
				<div class="heading">
					{#if editingSummary}
						<Textarea bind:value={title}></Textarea>
					{:else}
						<p class="text-15 text-bold">{branch.title}</p>
					{/if}
					<div class="actions">
						<Button icon="copy-small" kind="outline" onclick={copyLocation}>Share link</Button>
						{@render startReview(branch)}
						{#if branch.status === BranchStatus.Closed}
							<AsyncButton action={async () => updateStatus(BranchStatus.Active)} kind="outline"
								>Re-open review</AsyncButton
							>
						{:else}
							<AsyncButton
								style="error"
								kind="outline"
								action={async () => updateStatus(BranchStatus.Closed)}>Close review</AsyncButton
							>
						{/if}
					</div>
				</div>
				<InfoFlexRow>
					<Factoid label="Commits">
						<CommitsGraph {branch} />
					</Factoid>
					<Factoid label="Status"><BranchStatusBadge {branch} /></Factoid>
					<Factoid label="Authors">
						{#await contributors then contributors}
							<AvatarGroup avatars={contributors}></AvatarGroup>
						{/await}
					</Factoid>
					<Factoid label="Updated">
						{dayjs(branch.updatedAt).fromNow()}
					</Factoid>
					<Factoid label="Version">
						{branch.version}
					</Factoid>
				</InfoFlexRow>
				<div class="summary">
					{#if editingSummary}
						<Textarea minRows={6} bind:value={summary}></Textarea>
						<div class="summary-actions">
							<Button kind="outline" onclick={abortEditingSummary}>Cancel</Button>
							<AsyncButton style="pop" action={saveSummary}>Save</AsyncButton>
						</div>
					{:else if branch.description}
						<div class="text-13 summary-text">
							<Markdown content={branch.description} />
						</div>
						{#if branch.permissions.canWrite}
							<div>
								<Button kind="outline" onclick={editSummary}>Change details</Button>
							</div>
						{/if}
					{:else}
						<div class="summary-placeholder">
							<p class="text-13 text-clr2">No summary provided.</p>
							{#if branch.permissions.canWrite}
								<p class="text-12 text-body text-clr2">
									<em>
										Summaries provide context on the branch's purpose and helps team members
										understand it's changes.
									</em>
								</p>
								<Button icon="plus-small" kind="outline" onclick={editSummary}>Add summary</Button>
							{/if}
						</div>
					{/if}
				</div>
			</div>

			<div>
				<table class="commits-table">
					<thead>
						<tr>
							<th><div>Status</div></th>
							<th><div>Name</div></th>
							<th><div class="header-right">Changes</div></th>
							<th><div>Last update</div></th>
							<th><div>Authors</div></th>
							<th><div>Reviewers</div></th>
							<th><div>Comments</div></th>
						</tr>
					</thead>
					<tbody class="pretty">
						{#each branch.patchIds || [] as changeId, index}
							<ChangeIndexCard
								{changeId}
								params={data}
								branchUuid={branch.uuid}
								last={index === branch.patchIds.length - 1}
							/>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/snippet}
</Loading>

<style lang="postcss">
	.layout {
		display: grid;
		grid-template-columns: 6fr 10fr;
		gap: var(--layout-col-gap);

		@media (--desktop-small-viewport) {
			grid-template-columns: 1fr;
		}
	}

	.information {
		display: flex;
		gap: 24px;
		flex-direction: column;
		padding-right: 20px;

		@media (--tablet-viewport) {
			padding-right: 0;
		}
	}

	.heading {
		display: flex;
		gap: 16px;
		flex-direction: column;
	}

	.summary {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.summary-text {
		line-height: 160%; /* 20.8px */
	}

	.summary-placeholder {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 12px;
	}

	.header-right {
		text-align: right;
	}
</style>
