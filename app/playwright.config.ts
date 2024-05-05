import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: './e2e/playwright',
    testMatch: /(.+\.)?(test|spec)\.[jt]s/,
    reporter: process.env.CI ? 'list' : 'html',
    // reporter: process.env.CI
    //     ? [['dot'], ['json', { outputFile: 'test-results.json' }]]
    //     : [['list'], ['json', { outputFile: 'test-results.json' }], ['html', { open: 'on-failure' }]],
    // globalSetup: './e2e/playwright/globalSetup.ts',
    use: {
        launchOptions: {
            executablePath: '/home/ndo/.nix-profile/bin/chromium'
        },
        // baseURL: 'http://localhost:3000',
        trace: 'on-first-retry'
    },

    projects: [
        {
            name: 'Google Chrome',
            use: { ...devices['Desktop Chrome'] } // or 'chrome-beta'
        }
    ],

    webServer: {
        command: 'pnpm test:e2e:run',
        url: 'http://localhost:1420',
        reuseExistingServer: !process.env.CI
    }
});
