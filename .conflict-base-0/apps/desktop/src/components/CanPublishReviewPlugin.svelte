<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string | undefined;
		prNumber: number | undefined;
		reviewId: string | undefined;
	};

	const { projectId, stackId, branchName, prNumber, reviewId }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const stackService = getContext(StackService);

	const commits = $derived(
		stackId && branchName ? stackService.commits(projectId, stackId, branchName) : undefined
	);
	const branchEmpty = $derived(commits?.current.data ? commits.current.data.length === 0 : false);
	const prService = $derived(forge.current.prService);
	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const canPublishPR = $derived(forge.current.authenticated && !pr);

	const ctaLabel = 'Create Pull Request…';

	export const imports = {
		get allowedToPublishPR() {
			return forge.current.authenticated;
		},
		get branchIsEmpty() {
			return branchEmpty;
		},
		get branchIsConflicted() {
			return commits?.current.data?.some((commit) => commit.hasConflicts) || false;
		},
		get prNumber() {
			return prNumber;
		},
		get reviewId() {
			return reviewId;
		},
		get canPublishPR() {
			return canPublishPR;
		},
		get pr() {
			return pr;
		},
		get ctaLabel() {
			return ctaLabel;
		}
	};
</script>
