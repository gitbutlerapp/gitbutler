<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { inject } from "@gitbutler/core/context";
	import type { Commit } from "@gitbutler/but-sdk";

	type Props = {
		commits: Commit[];
		prNumber: number | undefined;
		reviewId: string | undefined;
	};

	const { commits, prNumber, reviewId }: Props = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);

	const branchEmpty = $derived(commits.length === 0);
	const prService = $derived(forge.current.prService);
	const prQuery = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prQuery?.response);

	const canPublishPR = $derived(forge.current.authenticated && !pr);

	const ctaLabel = $derived(`Create ${forge.reviewUnitName}…`);

	export const imports = {
		get allowedToPublishPR() {
			return forge.current.authenticated;
		},
		get branchIsEmpty() {
			return branchEmpty;
		},
		get branchIsConflicted() {
			return commits.some((commit) => commit.hasConflicts);
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
		},
	};
</script>
