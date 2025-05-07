# Frontend integration tests

These are the front-end-only integration tests, meaning that they run in the browser, use mocked backend data and only care about things like reactivity and graceful error handling.

## Stack

These tests use [Cypress](https://docs.cypress.io/app/get-started/why-cypress), which provides a nice setup of tools for development and testing web applications, especially when it comes to mocking data.

## Setup & Run locally

To run the e2e tests locally:

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

### 2. Open the application and run the tests

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

This should automatically pick up all the cypress tests. Click on them to run them.

### 3. Headless running

If you don't care about how pretty the application looks, and just want to check whether the tests are passing, you can run them headless:

```sh
pnpm cy:chrome
```

## Adding tests

The E2E tests are located under the following pattern `cypress/integration/**.cy.ts`.
Take a look at the other existing tests to see the general layout.

Note that new tests can easily be added through the Cypress GUI as well.

### Mocking tauri

If your application needs to mock some Tauri commands, take a look at the support file `cypress/integration/support/index.ts`.

There we're already doing some heavy lifting by adding some _default_ mocks. Feel free to add more mocks there if they apply to all tests globally.

**If you want to add test-specific mocks, though,** please use the `mockCommand` function inside your tests. This way we keep a 'clean state' as the default mocked state of the application for all tests.

### Initial State

Take a look at existing tests and their `beforeEach()` functions to find bits and pieces which might be suitable for your initial state. If a test suite, as enclosed in `describe()` functions, already has the desired initial state, the `it` function can be put there.

### Developing a test

During development, it helps to run only the test at hand using `it.only()`.

### Creating failing tests for later fixing

Once a UI bug was discovered, it makes sense to reproduce the issue in its own test case and hand it over for fixing. This can be done with `it.skip()`, such that the failing test won't fail CI until it is fixed.

## Extras

Cypress doesn't by default support testing in WebKit, and it still considers it's support as experimental.

If you want to try it out, though, you'll need to install it on your machine first:

```sh
pnpm cy:install-more-browsers
```

This will install a bunch of other browser drivers, including WebKit.
