<script lang="ts">
	import CommitsGraph from '../review/CommitsGraph.svelte';
	import BranchStatusBadge from '@gitbutler/shared/branches/BranchStatusBadge.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContributorsWithAvatars } from '@gitbutler/shared/branches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	type Props = {
		uuid: string;
		linkParams: ProjectParameters;
		roundedTop: boolean;
		roundedBottom: boolean;
	};

	const { uuid, linkParams, roundedTop, roundedBottom }: Props = $props();

	const appState = getContext(AppState);
	const branchService = getContext(BranchService);
	const routes = getContext(WebRoutesService);

	const branch = $derived(getBranchReview(appState, branchService, uuid));

	const contributors = $derived(
		isFound(branch.current) ? getContributorsWithAvatars(branch.current.value) : Promise.resolve([])
	);
</script>

<Loading loadable={branch.current}>
	{#snippet children(branch)}
		<tr class:rounded-top={roundedTop} class:rounded-bottom={roundedBottom} class="row">
			<td><div><BranchStatusBadge {branch} /></div></td>
			<td>
				<div class="text-13 text-bold title-column">
					<a
						title={branch.title}
						href={routes.projectReviewBranchPath({ ...linkParams, branchId: branch.branchId })}
					>
						{branch.title || '-'}
					</a>
				</div>
			</td>
			<td><div class="uuid">{branch.branchId.slice(0, 7)}</div></td>
			<td><div><CommitsGraph {branch} /></div></td>
			<td><div class="norm">{dayjs(branch.updatedAt).fromNow()}</div></td>
			<td>
				<div>
					{#await contributors then contributors}
						<AvatarGroup avatars={contributors}></AvatarGroup>
					{/await}
				</div>
			</td>
			<td><div class="norm">{branch.version || 0}</div></td>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
	.uuid {
		font-size: 0.8em;
		color: var(--clr-text-2);
		font-family: var(--fontfamily-mono);
	}

	.norm {
		font-size: 0.8em;
		color: var(--clr-text-2);
	}

	.title-column {
		width: auto;
	}

	.row {
		min-height: 50px;

		> td {
			padding: 0;
			height: 100%;

			> div {
				min-height: 50px;
				height: 100%;

				background-color: var(--clr-bg-1);
				padding: 16px;

				border-top: none;
				border-bottom: 1px solid var(--clr-border-2);

				white-space: nowrap;
				text-overflow: ellipsis;
				overflow: hidden;
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
