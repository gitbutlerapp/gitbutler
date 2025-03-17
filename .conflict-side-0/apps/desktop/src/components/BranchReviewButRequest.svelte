<script lang="ts">
	import { ButRequestDetailsService } from '$lib/forge/butRequestDetailsService';
	import { ProjectService } from '$lib/project/projectService';
	import { sleep } from '$lib/utils/sleep';
	import BranchStatusBadge from '@gitbutler/shared/branches/BranchStatusBadge.svelte';
	import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { inject } from '@gitbutler/shared/context';
	import { getContributorsWithAvatars } from '@gitbutler/shared/contributors';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { and, combine, isFound, isNotFound, map } from '@gitbutler/shared/network/loadable';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
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
		webRoutes,
		butRequestDetailsService
	] = inject(
		ProjectService,
		AppState,
		CloudProjectService,
		LatestBranchLookupService,
		CloudBranchService,
		WebRoutesService,
		ButRequestDetailsService
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

	const areAnyNotFound = $derived(anyNotFound());

	$effect(() => {
		const options = { keepPolling: true };
		if (areAnyNotFound) {
			pollWhileNotFound(reviewId, options);
		}

		return () => {
			options.keepPolling = false;
		};
	});

	$effect(() => {
		if (!isFound(cloudProject?.current)) return;
		if (!isFound(cloudBranch?.current)) return;

		butRequestDetailsService.updateDetails(
			cloudProject.current.value.owner,
			cloudProject.current.value.slug,
			cloudBranch.current.value.branchId
		);
	});

	async function pollWhileNotFound(reviewId: string, options: { keepPolling: boolean }) {
		let counter = 0;

		while (counter < 8 && options.keepPolling && untrack(() => anyNotFound())) {
			await sleep(100 * (2 ^ counter));

			await invalidateAll(reviewId);

			++counter;
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

	const contributors = $derived(
		isFound(cloudBranch?.current)
			? getContributorsWithAvatars(cloudBranch.current.value)
			: Promise.resolve([])
	);
</script>

{#if $project?.api?.repository_id}
	<Loading
		loadable={and([
			cloudBranchUuid?.current,
			combine([cloudBranch?.current, cloudProject?.current])
		])}
	>
		{#snippet children([cloudBranch, cloudProject])}
			<div class="br-overview">
				<div class="br-row">
					<Icon name="bowtie" />
					<Link
						target="_blank"
						rel="noreferrer"
						href={webRoutes.projectReviewBranchUrl({
							ownerSlug: cloudProject.owner,
							projectSlug: cloudProject.slug,
							branchId: cloudBranch.branchId
						})}
						externalIcon={false}
						class="br-link text-13">BR #{cloudBranch.branchId.slice(0, 4)}</Link
					>
					<BranchStatusBadge branch={cloudBranch}></BranchStatusBadge>
				</div>
				<div class="br-row">
					<div class="factoid text-12">
						<span class="label">Reviewers:</span>
						<div class="avatar-group-container">
							{#await contributors then contributors}
								<AvatarGroup avatars={contributors}></AvatarGroup>
							{/await}
						</div>
					</div>
					<span class="seperator">•</span>
					<div class="factoid text-12">
						<span class="label">Version:</span>
						{cloudBranch.version}
					</div>
				</div>
			</div>
		{/snippet}
	</Loading>
{/if}

<style lang="postcss">
	:global(.br-link) {
		text-decoration-style: dotted;
		text-decoration-thickness: 2px;
	}

	.br-overview {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.br-row {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.factoid {
		display: flex;
		align-items: center;
		gap: 4px;

		> .label {
			color: var(--clr-text-2);
		}
	}

	.seperator {
		transform: translateY(-1.5px);
		color: var(--clr-text-3);
	}

	.avatar-group-container {
		padding-right: 2px;
	}
</style>
