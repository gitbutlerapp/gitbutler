<script module lang="ts">
	import Badge from "$components/Badge.svelte";
	import ForgeUserCard from "$components/ForgeUserCard.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "List items / Forge User Card",
		component: ForgeUserCard,
		args: {
			username: "octocat",
			avatarUrl: "https://avatars.githubusercontent.com/u/583231?v=4",
			email: "octocat@github.com",
			isError: false,
			isLoading: false,
			isForgetLoading: false,
		},
		argTypes: {
			username: {
				control: { type: "text" },
			},
			avatarUrl: {
				control: { type: "text" },
			},
			email: {
				control: { type: "text" },
			},
			isError: {
				control: { type: "boolean" },
			},
			isLoading: {
				control: { type: "boolean" },
			},
			isForgetLoading: {
				control: { type: "boolean" },
			},
		},
	});
</script>

<Story name="Default">
	{#snippet template(args)}
		<ForgeUserCard
			username={args.username}
			avatarUrl={args.avatarUrl}
			email={args.email}
			isError={args.isError}
			isLoading={args.isLoading}
			isForgetLoading={args.isForgetLoading}
			onForget={() => alert("Forget clicked")}
		>
			{#snippet badge()}
				<Badge style="purple">PAT</Badge>
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</Story>

<Story name="Loading">
	{#snippet template()}
		<ForgeUserCard
			username="octocat"
			avatarUrl="https://avatars.githubusercontent.com/u/583231?v=4"
			email={null}
			isError={false}
			isLoading={true}
			isForgetLoading={false}
			onForget={() => alert("Forget clicked")}
		/>
	{/snippet}
</Story>

<Story name="Error">
	{#snippet template()}
		<ForgeUserCard
			username="octocat"
			avatarUrl="https://avatars.githubusercontent.com/u/583231?v=4"
			email={null}
			isError={true}
			isLoading={false}
			isForgetLoading={false}
			onForget={() => alert("Forget clicked")}
		/>
	{/snippet}
</Story>

<Story name="No Email">
	{#snippet template()}
		<ForgeUserCard
			username="octocat"
			avatarUrl="https://avatars.githubusercontent.com/u/583231?v=4"
			email={null}
			isError={false}
			isLoading={false}
			isForgetLoading={false}
			onForget={() => alert("Forget clicked")}
		/>
	{/snippet}
</Story>

<Story name="Forget Loading">
	{#snippet template()}
		<ForgeUserCard
			username="octocat"
			avatarUrl="https://avatars.githubusercontent.com/u/583231?v=4"
			email="octocat@github.com"
			isError={false}
			isLoading={false}
			isForgetLoading={true}
			onForget={() => alert("Forget clicked")}
		/>
	{/snippet}
</Story>

<Story name="No Avatar">
	{#snippet template()}
		<ForgeUserCard
			username="johndoe"
			avatarUrl={null}
			email="johndoe@example.com"
			isError={false}
			isLoading={false}
			isForgetLoading={false}
			onForget={() => alert("Forget clicked")}
		>
			{#snippet badge()}
				<Badge style="purple">PAT</Badge>
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</Story>

<Story name="GitLab">
	{#snippet template()}
		<ForgeUserCard
			username="gitlabuser"
			avatarUrl="https://secure.gravatar.com/avatar/e64c7d89f26bd1972efa854d13d7dd61?s=80&d=identicon"
			email="user@gitlab.com"
			isError={false}
			isLoading={false}
			isForgetLoading={false}
			onForget={() => alert("Forget clicked")}
		>
			{#snippet badge()}
				<Badge style="warning">PAT</Badge>
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</Story>

<Story name="Multiple Users">
	{#snippet template()}
		<div style="display: flex; flex-direction: column; gap: 16px;">
			<ForgeUserCard
				username="octocat"
				avatarUrl="https://avatars.githubusercontent.com/u/583231?v=4"
				email="octocat@github.com"
				isError={false}
				isLoading={false}
				isForgetLoading={false}
				onForget={() => alert("Forget octocat")}
			/>
			<ForgeUserCard
				username="gitlabuser"
				avatarUrl="https://secure.gravatar.com/avatar/e64c7d89f26bd1972efa854d13d7dd61?s=80&d=identicon"
				email="user@gitlab.com"
				isError={false}
				isLoading={false}
				isForgetLoading={false}
				onForget={() => alert("Forget gitlabuser")}
			/>
			<ForgeUserCard
				username="erroruser"
				avatarUrl={null}
				email={null}
				isError={true}
				isLoading={false}
				isForgetLoading={false}
				onForget={() => alert("Forget erroruser")}
			/>
		</div>
	{/snippet}
</Story>

<Story name="Playground" />
