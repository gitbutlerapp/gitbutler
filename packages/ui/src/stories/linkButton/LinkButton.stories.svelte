<script module lang="ts">
	import LinkButton from '$components/LinkButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Link Button',
		component: LinkButton,
		args: {
			icon: 'copy-small',
			onclick: () => {
				// eslint-disable-next-line no-console
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

<Story name="Playground">Link button</Story>
