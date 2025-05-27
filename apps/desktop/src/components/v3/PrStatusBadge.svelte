<script lang="ts">
	import PrStatusBadge, { type StatusType } from '@gitbutler/ui/PrStatusBadge.svelte';

	import type { DetailedPullRequest } from '$lib/forge/interface/types';

	interface Props {
		pr: DetailedPullRequest | undefined;
	}

	const { pr }: Props = $props();

	const prStatus: StatusType = $derived.by(() => {
		switch (true) {
			case !pr:
				return 'loading';
			case !!pr?.mergedAt:
				return 'merged';
			case !!pr?.closedAt:
				return 'closed';
			case !!pr?.draft:
				return 'draft';
			default:
				return 'open';
		}
	});
</script>

<PrStatusBadge status={prStatus} />
