<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { AIService, GitAIConfigKey, KeyOption, type DiffInput } from '$lib/ai/service';
	import { ModelKind, MessageRole, type Prompt } from '$lib/ai/types';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { slide } from 'svelte/transition';

	const aiService = getContext(AIService);
	const userService = getContext(UserService);
	const user = userService.user;

	let testing = $state(false);
	let result = $state<string | null>(null);
	let error = $state<string | null>(null);
	let modelKind = $state<ModelKind | undefined>();
	let isUsingButlerAPI = $state(false);
	let debugInfo = $state<string | null>(null);
	let showDebug = $state(false);

	// Simple test diff for commit message generation
	const testDiff: DiffInput[] = [
		{
			filePath: 'example.js',
			diff: `@@ -1,3 +1,5 @@
 function hello() {
-  return "Hello World";
+  // Add a greeting with the current time
+  const now = new Date();
+  return \`Hello World! The time is \${now.toLocaleTimeString()}\`;
 }`
		}
	];

	async function testAiCredentials() {
		testing = true;
		result = null;
		error = null;
		debugInfo = null;

		try {
			// Get current model kind
			modelKind = await aiService.getModelKind();
			console.log(`Testing AI credentials for model kind: ${modelKind}`);
			debugInfo = `Model kind: ${modelKind}`;

			// Check if using GitButler API
			isUsingButlerAPI = await aiService.usingGitButlerAPI();
			console.log(`Using GitButler API: ${isUsingButlerAPI}`);
			debugInfo += `, Using GB API: ${isUsingButlerAPI}`;

			// Check if configuration is valid
			const isConfigValid = await aiService.validateConfiguration();
			console.log(`Configuration valid: ${isConfigValid}`);
			debugInfo += `, Config valid: ${isConfigValid}`;

			if (!isConfigValid) {
				if (modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic) {
					if (isUsingButlerAPI && !$user) {
						throw new Error("Please sign in to use GitButler's AI API");
					} else {
						throw new Error('Please provide a valid API key for your selected AI service');
					}
				} else if (modelKind === ModelKind.Ollama) {
					// Get Ollama configuration for more detailed error
					const endpoint = await aiService.getOllamaEndpoint();
					const model = await aiService.getOllamaModelName();
					throw new Error(
						`Please check Ollama configuration: endpoint=${endpoint}, model=${model}`
					);
				}
			}

			console.log('Testing AI with commit message generation');
			debugInfo += `, Testing commit message generation`;

			// Use the summarizeCommit method with a timeout
			const summarizePromise = aiService.summarizeCommit({
				diffInput: testDiff,
				useEmojiStyle: false,
				useBriefStyle: false
			});

			console.log('Waiting for AI response...');
			const aiResult = await Promise.race([
				summarizePromise,
				new Promise<string>((_, reject) =>
					setTimeout(() => reject(new Error('AI response timed out after 8 seconds')), 8000)
				)
			]);

			// Set the result (handling undefined case)
			result = aiResult || null;

			console.log('Received commit message:', result);
			debugInfo += `, Received commit message: ${result?.substring(0, 30)}${result && result.length > 30 ? '...' : ''}`;

			// If result is empty or undefined, show an error
			if (!result || result.trim() === '') {
				throw new Error('Received empty response from AI service');
			}
		} catch (e) {
			console.error('AI credential check error:', e);
			error = e instanceof Error ? e.message : 'Unknown error occurred';
			debugInfo += `, Error: ${error}`;
		} finally {
			testing = false;
		}
	}

	function toggleDebug() {
		showDebug = !showDebug;
	}
</script>

<div class="ai-credential-check">
	{#if result || error}
		<div transition:slide={{ duration: 250 }}>
			<InfoMessage style={error ? 'warning' : 'success'} filled outlined={false}>
				{#snippet title()}
					{#if error}
						AI credential check failed
					{:else}
						AI credential check passed
					{/if}
				{/snippet}

				{#snippet content()}
					<div class="result-content" transition:slide={{ duration: 250 }}>
						{#if error}
							<div class="text-12 text-body error-text">
								<i class="result-icon">
									<Icon name="error-small" color="error" />
								</i>
								{error}
							</div>

							{#if (modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic) && isUsingButlerAPI && !$user}
								<div class="text-12 text-body help-text">
									<span> Please sign in to use GitButler's AI API. </span>
								</div>
							{:else if modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic}
								<div class="text-12 text-body help-text">
									<span> Please check your API key or try GitButler's API. </span>
								</div>
							{:else if modelKind === ModelKind.Ollama}
								<div class="text-12 text-body help-text">
									<span>
										Please check your Ollama endpoint and model configuration.
										<br />
										Make sure Ollama is running locally and accessible.
										<br />
										<Link href="https://ollama.ai">Learn more about Ollama</Link>
									</span>
								</div>
							{/if}
						{:else}
							<div class="text-12 text-body success-text">
								<i class="result-icon">
									<Icon name="success-small" color="success" />
								</i>
								<div class="ai-response">
									<strong>Sample commit message:</strong>
									<pre>{result}</pre>
								</div>
							</div>
						{/if}

						{#if showDebug && debugInfo}
							<div class="debug-info text-12">
								<hr />
								<div>Debug info: {debugInfo}</div>
							</div>
						{/if}
					</div>
				{/snippet}
			</InfoMessage>
		</div>
	{/if}
	<Button style="pop" wide icon="ai" disabled={testing} onclick={testAiCredentials}>
		{#if testing}
			Testing AI connection...
		{:else if result || error}
			Test again
		{:else}
			Test AI connection
		{/if}
	</Button>

	<div class="debug-toggle">
		<button class="text-12 debug-button" on:click={toggleDebug}>
			{showDebug ? 'Hide' : 'Show'} Debug Info
		</button>
	</div>
</div>

<style>
	.ai-credential-check {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.result-content {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 4px;
	}

	.result-icon {
		display: flex;
		align-items: flex-start;
		margin-right: 6px;
	}

	.error-text,
	.success-text {
		display: flex;
		align-items: flex-start;
	}

	.help-text {
		margin-top: 6px;
		margin-left: 18px;
	}

	.ai-response {
		max-height: 100px;
		overflow-y: auto;
		word-break: break-word;
	}

	.ai-response pre {
		margin: 8px 0 0 0;
		padding: 8px;
		background-color: var(--clr-bg-1);
		border-radius: 4px;
		font-family: var(--font-mono);
		white-space: pre-wrap;
		font-size: 12px;
	}

	.debug-toggle {
		display: flex;
		justify-content: flex-end;
	}

	.debug-button {
		background: none;
		border: none;
		color: var(--clr-text-3);
		cursor: pointer;
		padding: 4px 8px;
		text-decoration: underline;
		font-size: 11px;
	}

	.debug-info {
		margin-top: 8px;
		color: var(--clr-text-3);
		white-space: pre-wrap;
		word-break: break-word;
		font-family: monospace;
		font-size: 11px;
	}
</style>
