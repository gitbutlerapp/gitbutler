<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		gitHostPullRequestTemplatePath,
		gitHostUsePullRequestTemplate
	} from '$lib/config/config';
	import Section from '$lib/settings/Section.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';

	const usePullRequestTemplate = gitHostUsePullRequestTemplate();
	const pullRequestTemplatePath = gitHostPullRequestTemplatePath();
</script>

<Section>
	<svelte:fragment slot="title">Pull Request Template</svelte:fragment>
	<svelte:fragment slot="description">
		Use Pull Request template when creating a Pull Requests.
	</svelte:fragment>

	<div>
		<SectionCard
			roundedBottom={false}
			orientation="row"
			labelFor="use-pull-request-template-boolean"
		>
			<svelte:fragment slot="title">Enable Pull Request Templates</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="use-pull-request-template-boolean"
					value="false"
					bind:checked={$usePullRequestTemplate}
				/>
			</svelte:fragment>
			<svelte:fragment slot="caption">
				If enabled, we will use the path below to set the initial body of any pull requested created
				on this project through GitButler.
			</svelte:fragment>
		</SectionCard>
		<SectionCard roundedTop={false} orientation="row" labelFor="use-pull-request-template-path">
			<svelte:fragment slot="caption">
				<form>
					<fieldset class="fields-wrapper">
						<TextBox
							label="Pull request template path"
							id="use-pull-request-template-path"
							bind:value={$pullRequestTemplatePath}
							placeholder=".github/pull_request_template.md"
						/>
					</fieldset>
				</form>
			</svelte:fragment>
		</SectionCard>
	</div>
</Section>
<Spacer />
