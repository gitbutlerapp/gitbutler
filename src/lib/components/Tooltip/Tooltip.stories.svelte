<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import Tooltip from './Tooltip.svelte';
</script>

<Meta title="GitButler/Tooltip" component={Tooltip} />

<Story name="text with tooltip">
	<Tooltip label="This is a tooltip">
		<span class="text-zinc-50">Hover me!</span>
	</Tooltip>
</Story>
