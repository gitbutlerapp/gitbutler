<script lang="ts">
	import TableRow from '$lib/components/table/TableRow.svelte';
	import { inject } from '@gitbutler/core/context';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import {
		getPatchContributorsWithAvatars,
		getPatchApproversAllWithAvatars,
		getPatchRejectorsAllWithAvatars
	} from '@gitbutler/shared/contributors';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, isFound, map } from '@gitbutler/shared/network/loadable';
	import { getPatch } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import { getPatchStatus } from '@gitbutler/shared/patches/types';
	import {
		WEB_ROUTES_SERVICE,
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

	const routes = inject(WEB_ROUTES_SERVICE);

	const branch = $derived(getBranchReview(branchUuid));
	const change = $derived(getPatch(branchUuid, changeId));
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

	const currentPosition = $derived(
		map(combine([change.current, branch.current]), ([change, branch]) => {
			const patchCount = branch.patches?.length ?? 1;
			const patchDbPostion = change.position ?? 0;
			return patchCount - patchDbPostion;
		})
	);
</script>

<Loading loadable={combine([change.current, branch.current])}>
	{#snippet children([patch, branch])}
		<TableRow
			href={routes.projectReviewBranchCommitPath({ ...params, changeId: patch.changeId })}
			columns={[
				{ key: 'position', value: `${currentPosition}/${branch.patches?.length ?? 1}` },
				{ key: 'status', value: getPatchStatus(patch) },
				{ key: 'version', value: `v${patch.version}`, tooltip: 'Patch Version' },
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
			isTopEntry={currentPosition === branch.patches?.length}
			separatedBottom={currentPosition === 1}
		/>
	{/snippet}
</Loading>
