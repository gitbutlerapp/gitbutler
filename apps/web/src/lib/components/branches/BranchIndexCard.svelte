<script lang="ts">
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
	import {
		WebRoutesService,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	type Props = {
		repositoryId: string;
		uuid: string;
		linkParams: ProjectParameters;
		roundedTop: boolean;
		roundedBottom: boolean;
	};

	const { uuid, repositoryId, linkParams, roundedTop, roundedBottom }: Props = $props();

	const appState = getContext(AppState);
	const branchService = getContext(BranchService);
	const routes = getContext(WebRoutesService);

	const branch = $derived(getBranchReview(appState, branchService, repositoryId, uuid));

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
		<tr class:rounded-top={roundedTop} class:rounded-bottom={roundedBottom} class="row">
			<td><div>{branch.stackOrder}</div></td>
			<td>
				<div>
					<a href={routes.projectReviewBranchPath({ ...linkParams, branchId: branch.branchId })}>
						{branch.title}
					</a>
				</div>
			</td>
			<td><div>{branch.branchId.slice(0, 7)}</div></td>
			<td><div>{branch.stackSize}</div></td>
			<td>
				<div>
					{@render status(branch.status || BranchStatus.Active)}
				</div>
			</td>
			<td><div>{dayjs(branch.updatedAt).fromNow()}</div></td>
			<td>
				<div>
					{#await contributors then contributors}
						<AvatarGroup avatars={contributors}></AvatarGroup>
					{/await}
				</div>
			</td>
			<td><div>{branch.version || 0}</div></td>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
	.row {
		/*
			This is a magical incantation that lets the divs take up the full
			height of the cell. Nobody knows why this makes any difference
			because it's completly ingnored, but it does!
		*/
		height: 1px;

		> td {
			padding: 0;
			/* This is also part of the magical spell. */
			height: 1px;

			> div {
				height: 100%;

				background-color: var(--clr-bg-1);
				padding: 16px;

				border-top: none;
				border-bottom: 1px solid var(--clr-border-2);
			}

			&:first-child > div {
				border-left: 1px solid var(--clr-border-2);
			}

			&:last-child > div {
				border-right: 1px solid var(--clr-border-2);
			}
		}
	}

	.rounded-top > td {
		padding-top: 8px;

		> div {
			border-top: 1px solid var(--clr-border-2);
		}

		&:first-child > div {
			border-top-left-radius: var(--radius-m);
		}

		&:last-child > div {
			border-top-right-radius: var(--radius-m);
		}
	}

	.rounded-bottom > td {
		&:first-child > div {
			border-bottom-left-radius: var(--radius-m);
		}

		&:last-child > div {
			border-bottom-right-radius: var(--radius-m);
		}
	}
</style>
