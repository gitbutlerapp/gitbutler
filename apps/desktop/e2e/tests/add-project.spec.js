import { spawn } from 'node:child_process';
import { browser } from '@wdio/globals';

describe('Project', () => {
	before(() => {
		// Use 'for-listing.sh' helper to generate dummy repositories for test
		spawn('bash', ['e2e/scripts/init-repositories.sh', '../../target/release/gitbutler-cli']);
	});

	it('should add a local project', async () => {
		// 0. Accept Telemetry
		// TODO: Fix broken import in wdio
		// await handleTelemetryPage();
		const telemetryAgreement = await $('h1=Before we begin');
		await telemetryAgreement.waitForDisplayed();

		const acceptTelemetryBtn = await $('button=Continue');
		expect(acceptTelemetryBtn).toExist();
	});
});
