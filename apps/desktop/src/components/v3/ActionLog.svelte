<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ActionService from '$lib/actions/actionService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [actionService] = inject(ActionService);

	const actions = $derived(actionService.listActions(projectId));
</script>

<div class="action-log-wrap">
	<ReduxResult {projectId} result={actions.current}>
		{#snippet children(actions)}
			<div class="action-log">
				{#if actions.total > 0}
					<h2 class="text-16 text-semibold">Action Log</h2>
					<pre> {JSON.stringify(actions, null, 2)} </pre>
				{:else}
					<h2 class="text-16">No actions performed, yet!</h2>
				{/if}
			</div>
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.action-log-wrap {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.action-log {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 16px;
	}
</style>
