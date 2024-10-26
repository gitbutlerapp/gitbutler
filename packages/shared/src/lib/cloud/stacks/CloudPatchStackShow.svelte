<script lang="ts">
	import { CloudPatchStacksService } from '$lib/cloud/stacks/service';
	import { getContext } from '$lib/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);

	interface Props {
		patchStackId: string;
	}

	const { patchStackId }: Props = $props();

	const cloudPatchStacksService = getContext(CloudPatchStacksService);
	const optionalPatchStack = $derived(cloudPatchStacksService.patchStackForId(patchStackId));
</script>

{#if $optionalPatchStack.state === 'uninitialized'}
	<p>Loading...</p>
{:else if $optionalPatchStack.state === 'not-found'}
	<p>Error: Stack not found</p>
{:else if $optionalPatchStack.state === 'found'}
	{@const patchStack = $optionalPatchStack.value}

	<h1 class="text-head-24 padding-bottom">{patchStack.title}</h1>
	<div class="two-by-two padding-bottom">
		<div class="card">
			<div class="card__content">
				<p>Version: {patchStack.version}</p>
				<p>Status: {patchStack.status}</p>
				<p>Created at: {dayjs(patchStack.createdAt).fromNow()}</p>
			</div>
		</div>

		<div class="card">
			<p class="card__header text-15 text-bold">Contributors:</p>
			<div class="card__content">
				<ul>
					{#each patchStack.contributors as contributor}
						<li>{contributor}</li>
					{/each}
				</ul>
			</div>
		</div>
	</div>

	<h2 class="text-head-20 padding-bottom">Patches: ({patchStack.patches.length})</h2>

	<div class="card">
		{#each patchStack.patches as patch}
			<div class="line-item">
				<div>
					<p class="text-15 text-bold">{patch.title || 'Unnamed'}</p>
					<p>Commit: {patch.commitSha.slice(0, 7)} - Change: {patch.changeId.slice(0, 7)}</p>
					<p>Version: {patch.version}</p>
				</div>
				<Button style="pop" kind="solid">Visit</Button>
			</div>
		{/each}
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

	.line-item {
		padding: 8px;

		display: flex;
		justify-content: space-between;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-1);
		}
	}
</style>
