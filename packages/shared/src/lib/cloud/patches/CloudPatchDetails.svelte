<script lang="ts">
	import { CloudPatchService } from '$lib/cloud/patches/service';
	import { getContext } from '$lib/context';

	const cloudPatchService = getContext(CloudPatchService);
	const optionalPatch = cloudPatchService.patch;
</script>

{#if $optionalPatch.state === 'uninitialized'}
	<p>Loading...</p>
{:else if $optionalPatch.state === 'not-found'}
	<p>Failed to find patch</p>
{:else if $optionalPatch.state === 'found'}
	{@const patch = $optionalPatch.value}

	<h1 class="text-head-24 padding-bottom">{patch.title}</h1>

	<div class="two-by-two padding-bottom">
		<div class="card">
			<div class="card__content">
				<p>Version: {patch.version}</p>
				<p>Commit: {patch.commitSha.slice(0, 7)} - Change: {patch.changeId.slice(0, 7)}</p>
			</div>
		</div>

		<div class="card">
			<p class="card__header text-15 text-bold">Contributors:</p>
			<div class="card__content">
				<ul>
					{#each patch.contributors as contributor}
						<li>{contributor}</li>
					{/each}
				</ul>
			</div>
		</div>
		<div class="card">
			<p class="card__header text-15 text-bold">Reviews:</p>
			<div class="card__content">
				<p>Viewings: {patch.review.viewed}</p>
				<p>Sign offs: {patch.review.signedOff}</p>
				<p>Rejections: {patch.review.rejected}</p>
			</div>
		</div>
		<div class="card">
			<p class="card__header text-15 text-bold">Reviews (all revisions):</p>
			<div class="card__content">
				<p>Viewings: {patch.reviewAll.viewed}</p>
				<p>Sign offs: {patch.reviewAll.signedOff}</p>
				<p>Rejections: {patch.reviewAll.rejected}</p>
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.padding-bottom {
		margin-bottom: 16px;
	}

	.two-by-two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
	}
</style>
