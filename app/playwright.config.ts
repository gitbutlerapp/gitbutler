import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: './e2e/playwright',
    testMatch: /(.+\.)?(test|spec)\.[jt]s/,
    reporter: process.env.CI ? 'list' : 'html',
    use: process.CI
        ? { ...devices['Desktop Chrome'] }
        : {
            launchOptions: {
                executablePath: '/nix/store/6xi5mxm1yybq3a98n7m68cs0gdrx2bvd-chromium-124.0.6367.118/bin/chromium'
            },
            trace: 'on-first-retry'
        },
    projects: [
        {
            name: 'Google Chrome',
            use: { ...devices['Desktop Chrome'] }
        }
    ],
    webServer: {
        command: 'pnpm test:e2e:run',
        url: 'http://localhost:1420',
        reuseExistingServer: !process.env.CI
    }
});
