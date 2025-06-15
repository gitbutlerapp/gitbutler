<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { TemplateService } from '$lib/forge/templateService';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';

	interface Props {
		projectId: string;
		disabled: boolean;
		onselect: (template: string) => void;
	}

	let { projectId, disabled, onselect }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const templateService = getContext(TemplateService);

	// Available pull request templates.
	const templatesResult = $derived(templateService.getAvailable(forge.current.name));

	// The last template that was used. It is used as default if it is in the
	// list of available template.
	const templatePath = persisted<string | undefined>(undefined, `last-template-${projectId}`);
	const templateEnabled = persisted(false, `enable-template-${projectId}`);

	async function selectTemplate(path: string) {
		const template = await templateService.getContent(forge.current.name, path);
		templatePath.set(path);
		onselect(template);
	}

	async function setEnabled(enabled: boolean) {
		const ts = await templatesResult;
		templateEnabled.set(enabled);
		if (enabled) {
			const path = $templatePath ? $templatePath : ts.at(0);
			if (path) {
				selectTemplate(path);
			}
		}
	}
</script>

{#await templatesResult then templates}
	{#if templates.length > 0}
		<div class="pr-template__wrap">
			<label class="pr-template__toggle" for="pr-template-toggle">
				<span class="text-13 text-semibold">Use template</span>
				<Toggle
					small
					id="pr-template-toggle"
					onchange={(checked) => setEnabled(checked)}
					checked={$templateEnabled}
					disabled={templates.length === 0 || disabled}
				/>
			</label>
			<Select
				value={$templatePath}
				options={templates.map((value) => ({ label: value, value }))}
				placeholder={templates.length > 0 ? 'Choose template' : 'No PR templates found ¯\\_(ツ)_/¯'}
				flex="1"
				searchable
				disabled={!$templateEnabled || templates.length === 0 || disabled}
				onselect={(path) => selectTemplate(path)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === $templatePath} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
	{/if}
{/await}

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
