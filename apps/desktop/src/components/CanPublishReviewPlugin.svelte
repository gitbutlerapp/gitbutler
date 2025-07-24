<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string | undefined;
		prNumber: number | undefined;
		reviewId: string | undefined;
	};

	const { projectId, stackId, branchName, prNumber, reviewId }: Props = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const stackService = inject(STACK_SERVICE);

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
