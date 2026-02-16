<script module lang="ts">
	import { CardGroup } from '$lib/components/cardGroup';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Layout / CardGroup',
		component: CardGroup
	});
</script>

<Story name="Default">
	{#snippet template()}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					First Card Title
				{/snippet}
				{#snippet caption()}
					This is a caption for the first card
				{/snippet}
			</CardGroup.Item>

			<CardGroup.Item>
				{#snippet title()}
					Second Card Title
				{/snippet}
				{#snippet caption()}
					This is a caption for the second card
				{/snippet}
			</CardGroup.Item>

			<CardGroup.Item>
				{#snippet title()}
					Third Card Title
				{/snippet}
				{#snippet caption()}
					This is a caption for the third card
				{/snippet}
			</CardGroup.Item>
		</CardGroup>
	{/snippet}
</Story>

<Story name="With Content">
	{#snippet template()}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					Card with Custom Content
				{/snippet}
				{#snippet caption()}
					This card has additional content below
				{/snippet}
				<div style="padding: 12px; background: var(--clr-bg-2); border-radius: 6px;">
					Custom content can go here
				</div>
			</CardGroup.Item>

			<CardGroup.Item>
				{#snippet title()}
					Another Card
				{/snippet}
				{#snippet caption()}
					With more content
				{/snippet}
			</CardGroup.Item>
		</CardGroup>
	{/snippet}
</Story>

<Story name="With Actions">
	{#snippet template()}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					Card with Actions
				{/snippet}
				{#snippet caption()}
					This card has action buttons
				{/snippet}
				{#snippet actions()}
					<button type="button" class="btn btn-primary">Action</button>
				{/snippet}
			</CardGroup.Item>

			<CardGroup.Item>
				{#snippet title()}
					Another Card with Actions
				{/snippet}
				{#snippet actions()}
					<button type="button" class="btn btn-ghost">Cancel</button>
					<button type="button" class="btn btn-primary">Save</button>
				{/snippet}
			</CardGroup.Item>
		</CardGroup>
	{/snippet}
</Story>
