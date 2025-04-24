<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string | undefined;
		prNumber?: number;
		reviewId?: string;
	};

	const { projectId, stackId, branchName, prNumber, reviewId }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const stackService = getContext(StackService);
	const stackPublishingService = getContext(StackPublishingService);

	const branch = $derived(
		stackId && branchName ? stackService.branchByName(projectId, stackId, branchName) : undefined
	);

	const commits = $derived(
		stackId && branchName ? stackService.commits(projectId, stackId, branchName) : undefined
	);
	const branchEmpty = $derived(commits?.current.data ? commits.current.data.length === 0 : false);
	const name = $derived(branch?.current.data ? branch.current.data.name : undefined);
	const prService = $derived(forge.current.prService);
	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const canPublish = stackPublishingService.canPublish;
	const canPublishBR = $derived(!!($canPublish && name && !reviewId));
	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));

	const ctaLabel = $derived.by(() => {
		if (canPublishBR && canPublishPR) {
			return 'Submit for review…';
		} else if (canPublishBR) {
			return 'Create Butler Request…';
		} else if (canPublishPR) {
			return 'Create Pull Request…';
		}
		return 'Submit for review…';
	});

	export const imports = {
		get allowedToPublishPR() {
			return forge.current.authenticated;
		},
		get allowedToPublishBR() {
			return $canPublish;
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
		get canPublishBR() {
			return canPublishBR;
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
