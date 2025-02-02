<script lang="ts">
	import Factoid from '$lib/components/Factoid.svelte';
	import ChangeIndexCard from '$lib/components/changes/ChangeIndexCard.svelte';
	import BranchStatusBadge from '$lib/components/review/BranchStatusBadge.svelte';
	import CommitsGraph from '$lib/components/review/CommitsGraph.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { getContributorsWithAvatars, type Branch } from '@gitbutler/shared/branches/types';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, and, map } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import toasts from '@gitbutler/ui/toasts';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { goto } from '$app/navigation';
	import { PUBLIC_APP_HOST } from '$env/static/public';

	dayjs.extend(relativeTime);

	interface Props {
		data: ProjectReviewParameters;
	}

	let { data }: Props = $props();

	const latestBranchLookupService = getContext(LatestBranchLookupService);
	const branchService = getContext(BranchService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);

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

	function editSummary() {
		if (!isFound(branch?.current)) return;
		// Make sure we're not dealing with a reference to the origional
		summary = structuredClone(branch.current.value.description || '');
		editingSummary = true;
	}

	function abortEditingSummary() {
		if (!confirm('Canceling will loose any changes made')) {
			return;
		}

		editingSummary = false;
	}

	let savingSummary = $state<'inert' | 'loading' | 'complete'>('inert');

	async function saveSummary() {
		if (!isFound(branch?.current)) return;

		savingSummary = 'loading';

		try {
			await branchService.updateBranch(branch.current.value.uuid, {
				description: summary
			});
			toasts.success('Saved review summary');
		} finally {
			editingSummary = false;
			savingSummary = 'complete';
		}
	}

	function copyLocation() {
		copyToClipboard(location.href);
	}
</script>

{#snippet startReview(branch: Branch)}
	{#if (branch.stackSize || 0) > 0}
		<Button style="pop" icon="play" onclick={() => visitFirstCommit(branch)}>Start review</Button>
	{/if}
{/snippet}

<svelte:head>
	<title>Review: {data.ownerSlug}/{data.projectSlug}</title>
	<meta property="og:title" content="GitButler Review: {data.ownerSlug}/{data.projectSlug}" />
	<meta property="og:description" content="GitButler code review" />
	<meta
		property="og:image"
		content="{PUBLIC_APP_HOST}og/review/{data.ownerSlug}/{data.projectSlug}/{data.branchId}"
	/>
</svelte:head>

<Loading loadable={and([branchUuid?.current, branch?.current])}>
	{#snippet children(branch)}
		{console.log(branch)}
		<div class="layout">
			<div class="information">
				<div class="heading">
					<p class="text-15 text-bold">{branch.title}</p>
					<div class="actions">
						<!-- {#if !branch.description}
							<Button icon="plus-small" kind="outline" onclick={editSummary}>Add summary</Button>
						{/if} -->
						<Button icon="text-link" kind="outline" onclick={copyLocation}>Branch link</Button>
						{@render startReview(branch)}
					</div>
				</div>
				<div class="stats">
					<Factoid title="Commits:">
						<CommitsGraph {branch} />
					</Factoid>
					<Factoid title="Status:"><BranchStatusBadge {branch} /></Factoid>
					<Factoid title="Authors:">
						{#await contributors then contributors}
							<AvatarGroup avatars={contributors}></AvatarGroup>
						{/await}
					</Factoid>
					<Factoid title="Updated:">
						<p class="fact">{dayjs(branch.updatedAt).fromNow()}</p>
					</Factoid>
					<Factoid title="Version:">
						<p class="fact">{branch.version}</p>
					</Factoid>
				</div>
				<div class="summary">
					{#if editingSummary}
						<Textarea minRows={6} bind:value={summary}></Textarea>
						<div class="summary-actions">
							<Button
								kind="outline"
								onclick={abortEditingSummary}
								loading={savingSummary === 'loading'}>Cancel</Button
							>
							<Button style="pop" onclick={saveSummary} loading={savingSummary === 'loading'}
								>Save</Button
							>
						</div>
					{:else if branch.description}
						<Markdown content={branch.description} />
						<div>
							<Button kind="outline" onclick={editSummary}>Change summary</Button>
						</div>
					{:else}
						<div class="summary-placeholder">
							<p class="text-13 text-clr2">No summary provided.</p>
							<p class="text-12 text-body text-clr2">
								<em>
									Summaries provide context on the branch's purpose and helps team members
									understand it's changes.
								</em>
							</p>
							<Button icon="plus-small" kind="outline" onclick={editSummary}>Add summary</Button>
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
							<th><div>Changes</div></th>
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
		grid-template-columns: 5fr 11fr;
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

	.summary-placeholder {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 12px;
	}

	.stats {
		display: flex;
		flex-wrap: wrap;
		gap: 16px;
	}
</style>
