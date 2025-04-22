<script module lang="ts">
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Scrollable Container',
		component: ScrollableContainer,
		argTypes: {
			autoScroll: {
				defaultValue: true
			}
		}
	});

	const messages = $state([
		`Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nulla lacus mi, dignissim quis
		porttitor a, gravida volutpat diam. Maecenas sit amet tempus magna. Duis ac semper odio, ac
		vestibulum orci. Suspendisse vel nisi gravida, suscipit libero ut, placerat turpis. Ut eu
		rutrum magna, et elementum purus. Proin at volutpat lorem. Duis placerat ex in porttitor
		dignissim.`,
		`Aenean vitae turpis blandit, fermentum nisi id, convallis enim. Sed mattis, nibh at viverra
		iaculis, nulla tellus accumsan massa, ac luctus magna felis et justo. Nulla sit amet venenatis
		diam. Morbi eu interdum quam, sit amet bibendum turpis. Cras ultrices vestibulum ex sit amet
		tristique. Curabitur id lectus sed purus facilisis tempus. Proin aliquam tellus vel orci
		interdum, ut lobortis eros consectetur. Quisque quis ullamcorper purus. Interdum et malesuada
		fames ac ante ipsum primis in faucibus. Sed eget augue ac dolor hendrerit sagittis. Aliquam
		erat volutpat.`,
		`Vestibulum commodo aliquam ornare. Duis lobortis, lorem sit amet maximus convallis, neque
		lorem ornare nulla, ut tempus urna enim volutpat nisl. Proin tincidunt, odio vitae luctus
		congue, turpis arcu ultrices nibh, a posuere mi nisi sed urna. Nulla semper metus sit amet
		eros finibus, lobortis sodales neque egestas. Sed maximus, turpis ac tempor malesuada, arcu
		sapien aliquam sapien, sed iaculis lectus ligula vitae purus. Aliquam lacinia lorem at justo
		maximus, quis pharetra nunc facilisis. Cras facilisis cursus turpis. Fusce eget bibendum
		tellus. Integer cursus sapien nulla, nec lacinia dolor faucibus et.`
	]);
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<ScrollableContainer maxHeight="200px" whenToShow="hover" {...args} autoScroll>
		{#each messages as message}
			<p>{message}</p>
		{/each}
	</ScrollableContainer>
{/snippet}

<Story name="Playground" />

<style lang="postcss">
	p {
		white-space: 'pre';
		padding-bottom: 12px;
	}
</style>
