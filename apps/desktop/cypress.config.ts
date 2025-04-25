import { defineConfig } from 'cypress';

export default defineConfig({
	e2e: {
		baseUrl: 'http://localhost:1420',
		supportFile: 'cypress/e2e/support/index.ts'
	}
});
