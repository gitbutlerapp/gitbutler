<script lang="ts">
	import { ProjectService } from '$lib/project/projectService';
	import { sleep } from '$lib/utils/sleep';
	import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { inject } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { and, combine, isFound, isNotFound, map } from '@gitbutler/shared/network/loadable';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { untrack } from 'svelte';

	type Props = {
		reviewId: string;
	};

	const { reviewId }: Props = $props();

	const [
		projectService,
		appState,
		cloudProjectService,
		latestBranchLookupService,
		cloudBranchService,
		webRoutes
	] = inject(
		ProjectService,
		AppState,
		CloudProjectService,
		LatestBranchLookupService,
		CloudBranchService,
		WebRoutesService
	);

	const project = projectService.project;

	const cloudProject = $derived(
		$project?.api?.repository_id
			? getProjectByRepositoryId(appState, cloudProjectService, $project.api.repository_id)
			: undefined
	);

	const cloudBranchUuid = $derived(
		map(cloudProject?.current, (cloudProject) => {
			return lookupLatestBranchUuid(
				appState,
				latestBranchLookupService,
				cloudProject.owner,
				cloudProject.slug,
				reviewId
			);
		})
	);

	const cloudBranch = $derived(
		map(cloudBranchUuid?.current, (cloudBranchUuid) => {
			return getBranchReview(appState, cloudBranchService, cloudBranchUuid);
		})
	);

	$effect(() => {
		const options = { keepPolling: true };
		if (anyNotFound()) {
			pollWhileNotFound(reviewId, options);
		}

		return () => {
			options.keepPolling = false;
		};
	});

	async function pollWhileNotFound(reviewId: string, options: { keepPolling: boolean }) {
		let counter = 0;

		while (counter < 8 && options.keepPolling && untrack(() => anyNotFound())) {
			await sleep(100 * (2 ^ counter));

			await invalidateAll(reviewId);
		}
	}

	function anyNotFound() {
		return isNotFound(cloudBranchUuid?.current) || isNotFound(cloudBranch?.current);
	}

	async function invalidateAll(reviewId: string) {
		await untrack(async () => {
			if (!isFound(cloudProject?.current)) return;
			if (isNotFound(cloudBranchUuid?.current)) {
				await latestBranchLookupService.refreshBranchUuid(reviewId);
			}
			if (isFound(cloudBranchUuid?.current) && isNotFound(cloudBranch?.current)) {
				await cloudBranchService.refreshBranch(cloudBranchUuid.current.value);
			}
		});
	}
</script>

<Loading
	loadable={and([cloudBranchUuid?.current, combine([cloudBranch?.current, cloudProject?.current])])}
>
	{#snippet children([cloudBranch, cloudProject])}
		<Link
			target="_blank"
			rel="noreferrer"
			href={webRoutes.projectReviewBranchUrl({
				ownerSlug: cloudProject.owner,
				projectSlug: cloudProject.slug,
				branchId: cloudBranch.branchId
			})}
		>
			Open review</Link
		>
	{/snippet}
</Loading>
