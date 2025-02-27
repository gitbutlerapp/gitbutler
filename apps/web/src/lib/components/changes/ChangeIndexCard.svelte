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
	import Icon from '@gitbutler/ui/Icon.svelte';
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
		<tr class="row text-12" class:rounded-bottom={last}>
			<td> <div><CommitStatusBadge status={getPatchStatus(patch)} /></div></td>
			<td
				><div title={patch.title}>
					<a href={routes.projectReviewBranchCommitPath({ ...params, changeId: patch.changeId })}
						><p class="text-13 text-bold patch-name">{patch.title}</p></a
					>
				</div></td
			>
			<td
				><div class="row-text changes">
					<span class="changes_additions"
						>+{patch.statistics.lines - patch.statistics.deletions}</span
					>
					<span class="changes_deletions">-{patch.statistics.deletions}</span>
				</div></td
			>
			<td
				><div class="row-text updated" title={patch.updatedAt}>
					{dayjs(patch.updatedAt).fromNow()}
				</div></td
			>
			<td>
				<div>
					{#await contributors then contributors}
						<AvatarGroup avatars={contributors}></AvatarGroup>
					{/await}
				</div>
			</td>
			<td><div><PatchReviewersGroup {patch} /></div></td>
			<td
				><div class="row-text">
					<div class="comments" class:row-placeholder={!patch.commentCount}>
						<div class="comments-icon"><Icon name="comments-small" /></div>
						<div>{patch.commentCount}</div>
					</div>
				</div></td
			>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
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

	.row-text {
		text-wrap: nowrap;
	}

	.updated {
		color: var(--clr-text-2);
	}

	.comments {
		color: var(--clr-text-1);
		display: flex;
		gap: 4px;
		justify-content: center;
		align-items: center;
	}

	.comments-icon {
		display: flex;
		color: var(--clr-text-2);
	}

	.changes {
		display: flex;
		justify-content: flex-start;
	}
	.changes_additions {
		color: var(--clr-theme-succ-element);
		text-align: right;
	}
	.changes_deletions {
		color: var(--clr-theme-err-element);
		text-align: right;
		padding-left: 6px;
	}

	.patch-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
