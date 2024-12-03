<script module lang="ts">
	import LinkButton from '$lib/LinkButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Link Button',
		component: LinkButton,
		args: {
			icon: 'copy-small',
			onclick: () => {
				console.log('LinkButton clicked');
			}
		},
		argTypes: {
			icon: {
				control: {
					type: 'select'
				}
			}
		}
	});
</script>

<Story name="Default">Link button</Story>
