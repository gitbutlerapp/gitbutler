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

<div class="page">
	<div class="back-button">
		<Button
			onclick={() => {
				if ($repositoryId && cloudBranchId) {
					goto(routesService.cloudBranch($repositoryId, cloudBranchId));
				}
			}}>Back to branch</Button
		>
	</div>
	<div class="two-by-two">
		<div class="patch-container">
			<div>
				<CloudPatchDetails />
			</div>
			<div>
				<CloudPatchSections />
			</div>
		</div>
		<div>
			<p>I'm chat</p>
		</div>
	</div>
</div>

<style lang="postcss">
	.page {
		display: grid;
		grid-template-rows: 1fr auto;

		height: 100%;
		gap: 16px;

		margin: 24px;
		margin-bottom: 0;
	}

	.two-by-two {
		overflow: hidden;
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
	}

	.patch-container {
		flex-grow: 1;
		overflow: auto;

		height: 100%;
	}
</style>
