<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import Link from './Link.svelte';

	const content = [
		['https://gitbutler.com', 'External link'],
		['#', 'Internal link']
	] as [string, string][];
</script>

<Meta title="GitButler/Link" component={Link} />

<Story name="Basic Link">
	<div class="flex gap-2">
		<div class="flex flex-col gap-2">
			{#each content as [href, label]}
				<div class="flex gap-2">
					{#each [false, true] as disabled}
						<Link {href} target="_blank" role="basic" {disabled}>
							{label}
						</Link>
					{/each}
				</div>
			{/each}
		</div>
	</div>
</Story>

<Story name="Primary Link">
	<div class="flex gap-2">
		<div class="flex flex-col gap-2">
			{#each content as [href, label]}
				<div class="flex gap-2">
					{#each [false, true] as disabled}
						<Link {href} target="_blank" role="primary" {disabled}>
							{label}
						</Link>
					{/each}
				</div>
			{/each}
		</div>
	</div>
</Story>

<Story name="Destructive Link">
	<div class="flex gap-2">
		<div class="flex flex-col gap-2">
			{#each content as [href, label]}
				<div class="flex gap-2">
					{#each [false, true] as disabled}
						<Link {href} target="_blank" role="destructive" {disabled}>
							{label}
						</Link>
					{/each}
				</div>
			{/each}
		</div>
	</div>
</Story>
