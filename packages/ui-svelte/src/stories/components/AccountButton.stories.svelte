<script module lang="ts">
	import ProfileButton from "$components/avatar/ProfileButton.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Avatar / ProfileButton",
		component: ProfileButton,
		args: {
			srcUrl: "https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c",
			onclick: () => {
				// eslint-disable-next-line no-console
				console.log("Account button clicked");
			},
		},
	});
</script>

<Story name="Playground" />

<Story name="With Avatar">
	{#snippet template(args)}
		<ProfileButton
			srcUrl="https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c"
			onclick={args.onclick}
		/>
	{/snippet}
</Story>

<Story name="Without Avatar (Anonymous)">
	{#snippet template(args)}
		<ProfileButton srcUrl={null} onclick={args.onclick} />
	{/snippet}
</Story>
