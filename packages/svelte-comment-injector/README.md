# svelte-comment-injector

A Svelte preprocessor that injects an HTML comment at the top of every component, making it easier to identify components in browser DevTools.

## ✨ Features

- Injects a DevTools-visible HTML comment like:
  ```html
  <!-- Begin MyComponent.svelte -->
  <div class="my-component">
  	<!-- Component content -->
  </div>
  <!-- End MyComponent.svelte -->
  ```

## 📦 Installation

```bash
npm install svelte-devtools-comment --save-dev
```

## ⚙️ Usage

Add the preprocessor to your Svelte configuration:

```js
// svelte.config.js
import svelteDevtoolsComment from 'svelte-devtools-comment';

export default {
	preprocess: [svelteDevtoolsComment()]
	// ...rest of your config
};
```
