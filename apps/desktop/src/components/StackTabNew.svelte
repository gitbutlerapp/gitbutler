<script lang="ts">
	import { showError } from '$lib/notifications/toasts';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();
	const stackService = getContext(StackService);

	async function addNew() {
		const { data, error } = await stackService.new(projectId);
		if (data) {
			goto(stackPath(projectId, data.id));
		} else {
			showError('Failed to add new stack', error);
		}
	}
</script>

<button aria-label="new stack" type="button" class="new-stack" onclick={() => addNew()}>
	<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
		<path d="M0 10H20M10 0L10 20" stroke="currentColor" stroke-width="1.5" />
	</svg>
</button>

<style lang="postcss">
	.new-stack {
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 0 var(--radius-ml) 0 0;
		padding: 14px 20px;
		background: var(--clr-stack-tab-inactive);
		color: var(--clr-text-3);
		transition:
			color var(--transition-fast),
			background var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
			background: var(--clr-stack-tab-inactive-hover);
		}
	}
</style>
