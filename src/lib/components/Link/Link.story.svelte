<script lang="ts">
	import Link from './Link.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	const roles = [
		{ label: 'basic', value: 'basic' as const },
		{ label: 'primary', value: 'primary' as const },
		{ label: 'destructive', value: 'destructive' as const }
	];
	let targets = ['_blank', '_self', '_parent', '_top'] as const;

	let role = roles[0].value;
	let text = 'Link';
	let href = 'https://gitbutler.com';
	let disabled = false;
	let target = targets[0];
</script>

<Hst.Story title="Link">
	<svelte:fragment slot="controls">
		<Hst.Checkbox bind:value={disabled} title="disabled" />
		<Hst.Text bind:value={text} title="text" />
		<Hst.Radio options={roles} bind:value={role} title="role" />
		<Hst.Text bind:value={href} title="href" />
		<Hst.Select options={targets} bind:value={target} title="target" />
	</svelte:fragment>

	<Link {role} {href} {disabled}>
		{text}
	</Link>
</Hst.Story>
