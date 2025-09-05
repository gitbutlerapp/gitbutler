import { defineConfig } from 'cypress';

export default defineConfig({
	retries: {
		// Configure retry attempts for `cypress run`
		runMode: 2,
		// Configure retry attempts for `cypress open`
		openMode: 0
	},
	e2e: {
		experimentalRunAllSpecs: true,
		baseUrl: 'http://localhost:1420',
		supportFile: 'cypress/e2e/support/index.ts'
	},
	experimentalWebKitSupport: true,
	numTestsKeptInMemory: 10
});
