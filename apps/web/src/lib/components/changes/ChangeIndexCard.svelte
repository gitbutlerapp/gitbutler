<script lang="ts">
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
	import Badge from '@gitbutler/ui/Badge.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import type { ProjectReviewParameters } from '$lib/routing';

	dayjs.extend(relativeTime);

	type Props = {
		changeId: string;
		params: ProjectReviewParameters;
		branchUuid: string;
	};

	const { changeId, params: _params, branchUuid }: Props = $props();

	const appState = getContext(AppState);
	const patchService = getContext(PatchService);

	const change = $derived(getPatch(appState, patchService, branchUuid, changeId));
	const contributors = $derived(
		isFound(change.current)
			? getPatchContributorsWithAvatars(change.current.value)
			: Promise.resolve([])
	);
</script>

{#snippet status(status: 'approved' | 'changes-requested' | 'unreviewed' | 'in-discussion')}
	{#if status === 'approved'}
		<Badge>Approved</Badge>
	{:else if status === 'changes-requested'}
		<Badge>Changes Requested</Badge>
	{:else if status === 'unreviewed'}
		<Badge>Unreviewed</Badge>
	{:else if status === 'in-discussion'}
		<Badge>In Discussion</Badge>
	{/if}
{/snippet}

<Loading loadable={change.current}>
	{#snippet children(patch)}
		<tr class="row">
			<td><div>{@render status(getPatchStatus(patch))}</div></td>
			<td><div>{patch.title}</div></td>
			<td><div>{dayjs(patch.updatedAt).fromNow()}</div></td>
			<td><div>+{patch.statistics.lines} -{patch.statistics.deletions}</div></td>
			<td>
				<div>
					{#await contributors then contributors}
						<AvatarGroup avatars={contributors}></AvatarGroup>
					{/await}
				</div>
			</td>
			<td><div></div></td>
			<td><div></div></td>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
	.row {
		background-color: var(--clr-bg-1);
		overflow: hidden;

		> td {
			padding: 16px;

			border-top: 1px solid var(--clr-border-2);
			border-bottom: 1px solid var(--clr-border-2);

			&:first-child {
				border-left: 1px solid var(--clr-border-2);
			}

			&:last-child {
				border-right: 1px solid var(--clr-border-2);
			}
			> div {
				display: block;
			}
		}
	}
</style>
