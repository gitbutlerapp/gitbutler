<script lang="ts">
	import ReduxResult from '$components/shared/ReduxResult.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button } from '@gitbutler/ui';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const rulesService = inject(RULES_SERVICE);
	const [_, creatingRule] = rulesService.createWorkspaceRule;

	const rules = $derived(rulesService.listWorkspaceRules(projectId));
</script>

<div class="rules-list">
	<ReduxResult {projectId} result={rules.current}>
		{#snippet children(rules)}
			<div>
				{#each rules as rule (rule.id)}
					<pre class="text-12">
                        {JSON.stringify(rules, null, 2)}
                    </pre>
				{:else}
					<p class="text-12 text-grey">No rules found.</p>
				{/each}
			</div>
		{/snippet}
	</ReduxResult>
	<Button
		wide
		icon="plus-small"
		kind="outline"
		disabled
		tooltip="Not implemented yet"
		loading={creatingRule.current.isLoading}>Add new rule</Button
	>
</div>

<style lang="postcss">
	.rules-list {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 8px;
	}

	.text-grey {
		color: var(--clr-text-2);
	}
</style>
