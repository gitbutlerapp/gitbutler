<script lang="ts">
	import { InfoMessage, Link } from "@gitbutler/ui";

	import type { Code } from "@gitbutler/but-sdk";

	const { errorCode }: { errorCode: Code | undefined } = $props();
</script>

{#if errorCode === "GitHubOrgOAuthRestricted"}
	<InfoMessage style="warning" filled outlined={false}>
		{#snippet title()}
			Restricted by a GitHub organization
		{/snippet}
		{#snippet content()}
			An organization that owns this repository has blocked the GitButler OAuth app, so pull
			requests can't be listed or created right now. Ask an organization owner to approve the app,
			or connect an account that uses a personal access token — see the
			<Link
				href="https://docs.gitbutler.com/features/forge-integration/github-integration?utm_source=gitbutler-app&utm_medium=settings-banner&utm_campaign=org-oauth-restriction#connect-a-github-account"
				>docs</Link
			>.
		{/snippet}
	</InfoMessage>
{:else if errorCode === "GitHubOrgSamlRestricted"}
	<InfoMessage style="warning" filled outlined={false}>
		{#snippet title()}
			GitHub organization requires SAML SSO
		{/snippet}
		{#snippet content()}
			This repository's organization requires SAML SSO, but the selected GitHub credential isn't
			authorized for it. In GitHub, authorize the GitButler OAuth app or your personal access token
			for the organization, then try again.
		{/snippet}
	</InfoMessage>
{/if}
