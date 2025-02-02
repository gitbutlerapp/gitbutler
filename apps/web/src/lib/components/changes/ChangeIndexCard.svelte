<script lang="ts">
	import PatchReviewersGroup from '../review/PatchReviewersGroup.svelte';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import {
		getPatchContributorsWithAvatars,
		getPatchStatus
	} from '@gitbutler/shared/branches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import CommitStatusBadge from '@gitbutler/ui/CommitStatusBadge.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	type Props = {
		changeId: string;
		params: ProjectReviewParameters;
		branchUuid: string;
		last: boolean;
	};

	const { changeId, params, branchUuid, last }: Props = $props();

	const appState = getContext(AppState);
	const patchService = getContext(PatchService);
	const routes = getContext(WebRoutesService);

	const change = $derived(getPatch(appState, patchService, branchUuid, changeId));
	const contributors = $derived(
		isFound(change.current)
			? getPatchContributorsWithAvatars(change.current.value)
			: Promise.resolve([])
	);
</script>

<Loading loadable={change.current}>
	{#snippet children(patch)}
		<tr class="row" class:rounded-bottom={last}>
			<td> <div><CommitStatusBadge status={getPatchStatus(patch)} /></div></td>
			<td
				><div class="name">
					<a href={routes.projectReviewBranchCommitPath({ ...params, changeId: patch.changeId })}
						>{patch.title}</a
					>
				</div></td
			>
			<td><div>+{patch.statistics.lines} -{patch.statistics.deletions}</div></td>
			<td><div class="updated">{dayjs(patch.updatedAt).fromNow()}</div></td>
			<td>
				<div>
					{#await contributors then contributors}
						<AvatarGroup avatars={contributors}></AvatarGroup>
					{/await}
				</div>
			</td>
			<td><div><PatchReviewersGroup {patch} /></div></td>
			<td><div></div></td>
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

	.rounded-bottom > td {
		&:first-child > div {
			border-bottom-left-radius: var(--radius-m);
		}

		&:last-child > div {
			border-bottom-right-radius: var(--radius-m);
		}
	}

	.name {
		font-weight: bold;
	}

	.updated {
		color: var(--clr-text-2);
		font-size: 0.8rem;
	}
</style>
