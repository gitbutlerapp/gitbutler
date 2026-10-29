<script module lang="ts">
	import AddForgeAccountButton from "$components/AddForgeAccountButton.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Buttons / Add Forge Account Button",
		component: AddForgeAccountButton,
		args: {
			noAccounts: false,
			disabled: false,
			loading: false,
			menuItems: [
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			],
		},
		argTypes: {
			noAccounts: {
				control: { type: "boolean" },
			},
			disabled: {
				control: { type: "boolean" },
			},
			loading: {
				control: { type: "boolean" },
			},
		},
	});
</script>

<Story name="Default">
	{#snippet template(args)}
		<AddForgeAccountButton
			noAccounts={args.noAccounts}
			disabled={args.disabled}
			loading={args.loading}
			menuItems={args.menuItems}
		/>
	{/snippet}
</Story>

<Story name="No Accounts">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={true}
			disabled={false}
			loading={false}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="With Accounts">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={false}
			disabled={false}
			loading={false}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="Loading">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={false}
			disabled={false}
			loading={true}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="Disabled">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={false}
			disabled={true}
			loading={false}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="Single Menu Item">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={true}
			disabled={false}
			loading={false}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="Multiple Menu Items">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={false}
			disabled={false}
			loading={false}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="No Accounts Loading">
	{#snippet template()}
		<AddForgeAccountButton
			noAccounts={true}
			disabled={false}
			loading={true}
			menuItems={[
				{
					label: "GitHub",
					icon: "github",
					onclick: () => alert("GitHub clicked"),
				},
				{
					label: "GitLab",
					icon: "gitlab",
					onclick: () => alert("GitLab clicked"),
				},
			]}
		/>
	{/snippet}
</Story>

<Story name="Playground" />
