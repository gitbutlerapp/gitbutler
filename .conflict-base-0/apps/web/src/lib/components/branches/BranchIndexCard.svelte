<script lang="ts">
	import TableRow from '$lib/components/table/TableRow.svelte';
	import { inject } from '@gitbutler/core/context';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContributorsWithAvatars } from '@gitbutler/shared/contributors';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import {
		WEB_ROUTES_SERVICE,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	type Props = {
		uuid: string;
		linkParams: ProjectParameters;
		isTopEntry: boolean;
		roundedTop: boolean;
		roundedBottom: boolean;
	};

	const { uuid, linkParams, isTopEntry, roundedTop, roundedBottom }: Props = $props();

	const routes = inject(WEB_ROUTES_SERVICE);

	const branch = $derived(getBranchReview(uuid));

	let contributors = $state<Array<{ srcUrl: string; name: string }>>([]);

	$effect(() => {
		(async () => {
			contributors = isFound(branch.current)
				? await getContributorsWithAvatars(branch.current.value)
				: [];
		})();
	});
</script>

<Loading loadable={branch.current}>
	{#snippet children(branch)}
		<TableRow
			href={routes.projectReviewBranchPath({ ...linkParams, branchId: branch.branchId })}
			columns={[
				{ key: 'status', value: branch.reviewStatus },
				{ key: 'title', value: branch.title || '-', tooltip: branch.title },
				{ key: 'number', value: branch.branchId.slice(0, 7), tooltip: branch.branchId },
				{ key: 'commitGraph', value: { branch, ...linkParams } },
				{ key: 'date', value: branch.updatedAt },
				{ key: 'avatars', value: contributors },
				{ key: 'number', value: branch.version || 0 }
			]}
			{isTopEntry}
			separatedTop={roundedTop}
			separatedBottom={roundedBottom}
		/>
	{/snippet}
</Loading>
