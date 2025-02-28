<script lang="ts">
	import TableRow from '$lib/components/table/TableRow.svelte';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import {
		getPatchContributorsWithAvatars,
		getPatchStatus
	} from '@gitbutler/shared/branches/types';
	import {
		getPatchApproversAllWithAvatars,
		getPatchRejectorsAllWithAvatars
	} from '@gitbutler/shared/branches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	type Props = {
		changeId: string;
		params: ProjectReviewParameters;
		branchUuid: string;
		last: boolean;
	};

	const { changeId, params, branchUuid }: Props = $props();

	const appState = getContext(AppState);
	const patchService = getContext(PatchService);
	const routes = getContext(WebRoutesService);

	const change = $derived(getPatch(appState, patchService, branchUuid, changeId));
	let contributors = $state<Array<{ srcUrl: string; name: string }>>([]);
	let approvers = $state<Array<{ srcUrl: string; name: string }>>([]);
	let rejectors = $state<Array<{ srcUrl: string; name: string }>>([]);

	$effect(() => {
		(async () => {
			contributors = isFound(change.current)
				? await getPatchContributorsWithAvatars(change.current.value)
				: [];

			approvers = isFound(change.current)
				? await getPatchApproversAllWithAvatars(change.current.value)
				: [];

			rejectors = isFound(change.current)
				? await getPatchRejectorsAllWithAvatars(change.current.value)
				: [];
		})();
	});
</script>

<Loading loadable={change.current}>
	{#snippet children(patch: any)}
		<TableRow
			href={routes.projectReviewBranchCommitPath({ ...params, changeId: patch.changeId })}
			columns={[
				{ key: 'status', value: getPatchStatus(patch) },
				{ key: 'title', value: patch.title, tooltip: patch.title },
				{
					key: 'changes',
					value: {
						additions: patch.statistics.lines - patch.statistics.deletions,
						deletions: patch.statistics.deletions
					}
				},
				{ key: 'date', value: patch.updatedAt, tooltip: patch.updatedAt },
				{ key: 'avatars', value: contributors },
				{ key: 'reviewers', value: { approvers, rejectors } },
				{ key: 'comments', value: patch.commentCount, tooltip: 'Comments' }
			]}
		/>
	{/snippet}
</Loading>
