<script module lang="ts">
	import ButPcAvatar from '$components/ButPcAvatar.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Avatar / ButPcAvatar',
		component: ButPcAvatar,
		args: {}
	});
</script>

<Story name="Default" />
