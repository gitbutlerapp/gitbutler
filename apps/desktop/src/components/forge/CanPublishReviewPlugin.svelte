<script lang="ts">
	import { useForgeAuth } from "$lib/forge/forgeAuth.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import type { Commit } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		commits: Commit[];
		prNumber: number | undefined;
		reviewId: string | undefined;
	};

	const { projectId, commits, prNumber, reviewId }: Props = $props();

	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const auth = useForgeAuth(reactive(() => projectId));

	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const reviewUnitName = $derived(forgeInfo?.unit.name ?? "Pull request");

	const branchEmpty = $derived(commits.length === 0);
	const prQuery = $derived(prNumber ? prService.get(projectId, prNumber) : undefined);
	const pr = $derived(prQuery?.response);

	const canPublishPR = $derived(auth.authenticated.current && !pr);

	const ctaLabel = $derived(`Create ${reviewUnitName}…`);

	export const imports = {
		get allowedToPublishPR() {
			return auth.authenticated.current;
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
