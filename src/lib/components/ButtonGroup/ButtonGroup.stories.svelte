<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import ButtonGroup from './ButtonGroup.svelte';

	const noop = () => {};
</script>

<Meta title="GitButler/ButtonGroup" component={ButtonGroup} />

<Story name="Two buttons">
	<ButtonGroup leftLabel="Cancel" rightLabel="Submit" leftAction={noop} rightAction={noop} />
</Story>

<Story name="Two buttons wide">
	<ButtonGroup
		leftLabel="Cancel"
		rightLabel="Submit"
		width="full-width"
		leftAction={noop}
		rightAction={noop}
	/>
</Story>

<Story name="Three buttons">
	<ButtonGroup
		leftLabel="Cancel"
		rightLabel="Submit"
		middleLabel="Middle"
		leftAction={noop}
		rightAction={noop}
	/>
</Story>

<Story name="Three buttons wide">
	<ButtonGroup
		leftLabel="Cancel"
		rightLabel="Submit"
		middleLabel="Middle"
		width="full-width"
		leftAction={noop}
		rightAction={noop}
	/>
</Story>
