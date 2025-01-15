<script lang="ts">
	import { projectReviewBranchPath, type ProjectParameters } from '$lib/routing';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import {
		getBranchReview,
		getContributorsWithAvatars
	} from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { BranchStatus } from '@gitbutler/shared/branches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	type Props = {
		repositoryId: string;
		branchId: string;
		linkParams: ProjectParameters;
		roundedTop: boolean;
		roundedBottom: boolean;
	};

	const { branchId, repositoryId, linkParams, roundedTop, roundedBottom }: Props = $props();

	const appState = getContext(AppState);
	const branchService = getContext(BranchService);

	const branch = $derived(getBranchReview(appState, branchService, repositoryId, branchId));

	const contributors = $derived(
		isFound(branch.current) ? getContributorsWithAvatars(branch.current.value) : Promise.resolve([])
	);
</script>

{#snippet status(status: BranchStatus)}
	{#if status === BranchStatus.Active}
		<Badge>Active</Badge>
	{:else if status === BranchStatus.Inactive}
		<Badge>Inactive</Badge>
	{:else if status === BranchStatus.Closed}
		<Badge>Closed</Badge>
	{:else if status === BranchStatus.Loading}
		<Badge>Processing</Badge>
	{/if}
{/snippet}

<Loading loadable={branch.current}>
	{#snippet children(branch)}
		<tr class:roundedTop class:roundedBottom class="row">
			<td>1</td>
			<td>
				<a href={projectReviewBranchPath({ ...linkParams, branchId })}>
					{branch.title}
				</a>
			</td>
			<td>{branch.branchId.slice(0, 7)}</td>
			<td>{branch.stackSize}</td>
			<td>
				{@render status(branch.status || BranchStatus.Active)}
			</td>
			<td>{dayjs(branch.updatedAt).fromNow()}</td>
			<td>
				{#await contributors then contributors}
					<AvatarGroup avatars={contributors}></AvatarGroup>
				{/await}
			</td>
			<td>{branch.version || 0}</td>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
	.row {
		background-color: var(--clr-bg-1);
		overflow: hidden;
		border-radius: 16px;

		> td {
			padding: 16px;

			border-top: 1px solid var(--clr-border-2);
			border-bottom: 1px solid var(--clr-border-2);

			&:first-child {
				border-left: 1px solid var(--clr-border-2);
				border-top-left-radius: var(--radius-m);
			}

			&:last-child {
				border-right: 1px solid var(--clr-border-2);
				border-top-right-radius: var(--radius-m);
			}
		}
	}
</style>
