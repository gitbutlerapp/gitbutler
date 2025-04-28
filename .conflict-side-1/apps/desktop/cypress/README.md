# Frontend integration tests

These are the front-end-only integration tests, meaning that they run in the browser, use mocked backend data and only care about things like reactivity and graceful error handling.

## Stack

These tests use [Cypress](https://docs.cypress.io/app/get-started/why-cypress), which provides a nice setup of tools for development and testing web applications, specially when it comes to mocking data.

## Setup & Run locally

In order to run the e2e tests locally:

### 1. Ensure that you have the Cypress dependencies installed

Install all `@gitbutler/desktop` pnpm deps

```sh
pnpm install
```

Install the Cypress app

```sh
pnpm cy:install
```

This will install the test application controller.

### 2. Open the application & run the tests

Run the GitButler's front-end application dev server:

```sh
pnpm dev
```

In another terminal, open the application:

```sh
pnpm cy:open
```

This should open the Cypress desktop application, offering two options:

1. Component testing
2. E2E testing

Open the `E2E testing` option.

This should automatically pick-up all the cypress tests. Click on them to run them.

### 3. Headless running

If you don't care about how pretty the application looks, and just want to check whether the tests are passing, you can run them headless:

```sh
pnpm cy:chrome
```

## Adding tests

The E2E tests are located under the following pattern `cypress/integration/**.cy.ts`.
Take a look at the other existing tests in order to see the general layout.

### Mocking tauri

If your application needs to mock some Tauri commands, take a look at the support file `cypress/integration/support/index.ts`.

There we're already doing some heavy lifting by adding some _default_ mocks. Feel free to add more mocks there if they apply to all tests globally.

**If you want to add test-specific mocks, though,** please use the `mockCommand` function inside your tests. This way we keep a 'clean state' as the default mocked state of the application for all tests.
