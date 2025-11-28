<script module lang="ts">
	import { Section } from '$lib/components/section';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Layout / Section',
		component: Section
	});
</script>

<Story name="Default">
	<Section>
		<Section.Card>
			{#snippet title()}
				First Card Title
			{/snippet}
			{#snippet caption()}
				This is a caption for the first card
			{/snippet}
		</Section.Card>

		<Section.Card>
			{#snippet title()}
				Second Card Title
			{/snippet}
			{#snippet caption()}
				This is a caption for the second card
			{/snippet}
		</Section.Card>

		<Section.Card>
			{#snippet title()}
				Third Card Title
			{/snippet}
			{#snippet caption()}
				This is a caption for the third card
			{/snippet}
		</Section.Card>
	</Section>
</Story>

<Story name="With Content">
	<Section>
		<Section.Card>
			{#snippet title()}
				Card with Custom Content
			{/snippet}
			{#snippet caption()}
				This card has additional content below
			{/snippet}
			<div style="padding: 12px; background: var(--clr-bg-2); border-radius: 6px;">
				Custom content can go here
			</div>
		</Section.Card>

		<Section.Card>
			{#snippet title()}
				Another Card
			{/snippet}
			{#snippet caption()}
				With more content
			{/snippet}
		</Section.Card>
	</Section>
</Story>

<Story name="With Actions">
	<Section>
		<Section.Card>
			{#snippet title()}
				Card with Actions
			{/snippet}
			{#snippet caption()}
				This card has action buttons
			{/snippet}
			{#snippet actions()}
				<button type="button" class="btn btn-primary">Action</button>
			{/snippet}
		</Section.Card>

		<Section.Card>
			{#snippet title()}
				Another Card with Actions
			{/snippet}
			{#snippet actions()}
				<button type="button" class="btn btn-ghost">Cancel</button>
				<button type="button" class="btn btn-primary">Save</button>
			{/snippet}
		</Section.Card>
	</Section>
</Story>
