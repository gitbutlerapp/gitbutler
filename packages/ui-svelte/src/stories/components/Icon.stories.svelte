<script module lang="ts">
	import Icon, { allIconNames } from "$components/Icon.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	import type { IconName } from "$components/Icon.svelte";

	const { Story } = defineMeta({
		title: "Basic / Icon",
		component: Icon,
		args: {
			name: "pr" as IconName,
			size: 16,
			color: "currentColor",
		},
		argTypes: {
			name: {
				options: allIconNames,
				control: { type: "select" },
			},
			size: {
				control: { type: "number" },
			},
			color: {
				control: { type: "color" },
			},
		},
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<Icon name={args.name} size={args.size} color={args.color} />
	{/snippet}
</Story>

<Story name="All Icons">
	{#snippet template(args)}
		<div class="icons">
			{#each allIconNames as name}
				<div class="icon-item">
					<Icon {name} size={args.size} color={args.color} />
					<span class="text-11 icon-label">{name}</span>
				</div>
			{/each}
		</div>
	{/snippet}
</Story>

<style>
	.icons {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
		padding: 32px;
		gap: 16px;
	}

	.icon-item {
		display: flex;
		flex-direction: column;
		gap: 12px;
		color: var(--text-1);
	}

	.icon-label {
		color: var(--text-2);
	}
</style>
