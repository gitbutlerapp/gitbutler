<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import type { Writable } from 'svelte/store';

	interface Props {
		projectId: string;
		forgeName: string;
		disabled: boolean;
		template: {
			enabled: Writable<boolean>;
			path: Writable<string | undefined>;
		};
		onselect: (template: string) => void;
	}

	const { projectId, forgeName, template, disabled, onselect }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	const path = $derived(template.path);
	const enabled = $derived(template.enabled);

	// Available pull request templates.
	const templatesResult = $derived(stackService.templates(projectId, forgeName));

	async function selectTemplate(newPath: string) {
		const template = await stackService.template(projectId, forgeName, newPath);
		if (template) {
			path.set(newPath);
			onselect(template);
		}
	}

	async function setEnabled(value: boolean) {
		const ts = templatesResult;
		enabled.set(value);
		if (value) {
			const path = $path ? $path : ts.current?.data?.at(0);
			if (path) {
				selectTemplate(path);
			}
		}
	}
</script>

<ReduxResult {projectId} result={templatesResult.current}>
	{#snippet children(templates)}
		{#if templates && templates.length > 0}
			<div class="pr-template__wrap">
				<label class="pr-template__toggle" for="pr-template-toggle">
					<span class="text-13 text-semibold">Use template</span>
					<Toggle
						testId={TestId.ReviewTemplateToggle}
						small
						id="pr-template-toggle"
						onchange={(checked) => setEnabled(checked)}
						checked={$enabled}
						disabled={templates.length === 0 || disabled}
					/>
				</label>
				<Select
					value={$path}
					options={templates.map((value) => ({ label: value, value }))}
					placeholder={templates.length > 0
						? 'Choose template'
						: 'No PR templates found ¯\\_(ツ)_/¯'}
					flex="1"
					searchable
					disabled={!$enabled || templates.length === 0 || disabled}
					onselect={(path) => selectTemplate(path)}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === $path} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.pr-template__wrap {
		display: flex;
		gap: 4px;
	}

	.pr-template__toggle {
		display: flex;
		align-items: center;
		padding: 8px 10px;
		gap: 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}
</style>
