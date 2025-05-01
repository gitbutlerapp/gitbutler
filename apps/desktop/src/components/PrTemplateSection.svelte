<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { TemplateService } from '$lib/forge/templateService';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';

	interface Props {
		templates: string[];
		selectedTemplate: string | undefined;
		disabled: boolean;
	}

	let { templates, disabled, selectedTemplate = $bindable() }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	// TODO: Rename or refactor this service.
	const templateService = getContext(TemplateService);
	const project = getContext(Project);

	// The last template that was used. It is used as default if it is in the
	// list of available template.
	const lastTemplate = persisted<string | undefined>(undefined, `last-template-${project.id}`);

	function handleToggle() {
		if (selectedTemplate) {
			setTemplate(undefined);
			return;
		}

		if (templates.length === 0) {
			return;
		}

		const path = templates.at(0);
		if (!path) {
			// Should not happen.
			return;
		}
		setTemplate(path);
	}

	async function setTemplate(path: string | undefined) {
		lastTemplate.set(path);
		loadAndEmit(path);
	}

	async function loadAndEmit(path: string | undefined) {
		if (path) {
			const template = await templateService.getContent(forge.current.name, path);
			if (template) {
				selectedTemplate = template;
			}
			return;
		}

		selectedTemplate = undefined;
	}
</script>

<div class="pr-template__wrap">
	<label class="pr-template__toggle" for="pr-template-toggle">
		<span class="text-13 text-semibold">Use template</span>
		<Toggle
			id="pr-template-toggle"
			checked={!!selectedTemplate}
			disabled={templates.length === 0 || disabled}
			onclick={handleToggle}
		/>
	</label>
	<Select
		value={$lastTemplate}
		options={templates.map((value) => ({ label: value, value }))}
		placeholder={templates.length > 0 ? 'Choose template' : 'No PR templates found ¯\\_(ツ)_/¯'}
		flex="1"
		searchable
		disabled={templates.length === 0 || !selectedTemplate || disabled}
		onselect={setTemplate}
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === $lastTemplate} {highlighted}>
				{item.label}
			</SelectItem>
		{/snippet}
	</Select>
</div>

<style lang="postcss">
	.pr-template__wrap {
		display: flex;
		gap: 4px;
	}

	.pr-template__toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		padding: 8px 10px;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}
</style>
