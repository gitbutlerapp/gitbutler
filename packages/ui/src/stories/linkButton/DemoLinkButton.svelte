<script lang="ts">
	import LinkButton from '$lib/LinkButton.svelte';
	import iconsJson from '$lib/data/icons.json';

	interface Props {
		icon?: keyof typeof iconsJson;
		onclick: (event?: any) => void;
	}

	const { icon, onclick }: Props = $props();
</script>

<LinkButton {icon} {onclick}>Link button</LinkButton>
