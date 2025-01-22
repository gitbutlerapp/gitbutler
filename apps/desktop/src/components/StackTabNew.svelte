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
		<g clip-path="url(#clip0_3856_9495)">
			<path d="M0 10H20M10 0L10 20" stroke="#867E79" stroke-width="1.5" />
		</g>
		<defs>
			<clipPath id="clip0_3856_9495">
				<rect width="20" height="20" fill="white" />
			</clipPath>
		</defs>
	</svg>
</button>

<style lang="postcss">
	.new-stack {
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 0 10px 0 0;
		padding: 14px 20px;
		&:hover {
			background: var(--clr-stack-tab-active);
		}
	}
</style>
