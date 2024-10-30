<script lang="ts">
	import CloudPatchDetails from '@gitbutler/shared/cloud/patches/CloudPatchDetails.svelte';
	import CloudPatchSections from '@gitbutler/shared/cloud/patches/CloudPatchSections.svelte';
	import { CloudBranchesService } from '@gitbutler/shared/cloud/stacks/service';
	import { getContext } from '@gitbutler/shared/context';
	import { getRoutesService } from '@gitbutler/shared/sharedRoutes';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const cloudBranchesService = getContext(CloudBranchesService);
	const repositoryId = cloudBranchesService.repositoryId;
	const cloudBranchId = $derived($page.params.cloudBranchId);

	const routesService = getRoutesService();
</script>

<div class="patch-container">
	<div class="back-button">
		<Button
			onclick={() => {
				if ($repositoryId && cloudBranchId) {
					goto(routesService.cloudBranch($repositoryId, cloudBranchId));
				}
			}}>Back to branch</Button
		>
	</div>
	<div>
		<CloudPatchDetails />
	</div>
	<div>
		<CloudPatchSections />
	</div>
</div>

<style lang="postcss">
	.patch-container {
		display: flex;
		flex-direction: column;

		width: 100%;

		margin: 24px auto;
		margin-bottom: 0;

		gap: 16px;

		overflow: auto;
	}
</style>
